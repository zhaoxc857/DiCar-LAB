param(
    [switch]$Publish
)

$ErrorActionPreference = "Stop"
$owner = "zhaoxc857"
$repo = "DiCar_Tune"
$tagName = "v1.7.0"
$apiRoot = "https://api.github.com/repos/$owner/$repo"

function Invoke-GitHubApi {
    param(
        [Parameter(Mandatory)] [ValidateSet("GET", "POST", "PATCH")] [string]$Method,
        [Parameter(Mandatory)] [string]$Uri,
        [object]$Body
    )

    $parameters = @{
        Method = $Method
        Uri = $Uri
        Headers = $script:headers
        TimeoutSec = 30
    }
    if ($null -ne $Body) {
        $parameters.ContentType = "application/json"
        $parameters.Body = $Body | ConvertTo-Json -Depth 12 -Compress
    }
    Invoke-RestMethod @parameters
}

function Test-GitHubResource {
    param([Parameter(Mandatory)] [string]$Uri)
    try {
        Invoke-GitHubApi -Method GET -Uri $Uri | Out-Null
        return $true
    } catch {
        if ($_.Exception.Response.StatusCode -eq 404) {
            return $false
        }
        throw
    }
}

function Get-GitBlobBytes {
    param([Parameter(Mandatory)] [string]$ObjectId)

    $startInfo = [Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = (Get-Command git).Source
    $startInfo.ArgumentList.Add("cat-file")
    $startInfo.ArgumentList.Add("blob")
    $startInfo.ArgumentList.Add($ObjectId)
    $startInfo.UseShellExecute = $false
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true

    $process = [Diagnostics.Process]::Start($startInfo)
    $memory = [IO.MemoryStream]::new()
    try {
        $process.StandardOutput.BaseStream.CopyTo($memory)
        $errorText = $process.StandardError.ReadToEnd()
        $process.WaitForExit()
        if ($process.ExitCode -ne 0) {
            throw "git cat-file failed: $errorText"
        }
        return ,$memory.ToArray()
    } finally {
        $memory.Dispose()
        $process.Dispose()
    }
}

$credentialLines = @("protocol=https", "host=github.com", "") |
    git credential-manager get 2>$null
$tokenLine = $credentialLines | Where-Object { $_ -like "password=*" } | Select-Object -First 1
if (-not $tokenLine) {
    throw "GitHub credential is unavailable in Git Credential Manager."
}
$token = $tokenLine.Substring("password=".Length)
$credentialLines = $null
$script:headers = @{
    Accept = "application/vnd.github+json"
    Authorization = "Bearer $token"
    "X-GitHub-Api-Version" = "2022-11-28"
    "User-Agent" = "DiCAR-LAB-release"
}

try {
    $root = (git rev-parse --show-toplevel).Trim()
    $head = (git rev-parse HEAD).Trim()
    $base = (git rev-parse origin/main).Trim()
    $localTree = (git rev-parse "$head`^{tree}").Trim()
    $remoteRef = Invoke-GitHubApi -Method GET -Uri "$apiRoot/git/ref/heads/main"
    $remoteHead = $remoteRef.object.sha

    if ($remoteHead -ne $base) {
        throw "Remote main moved: expected $base, found $remoteHead."
    }
    if (Test-GitHubResource -Uri "$apiRoot/git/ref/tags/$tagName") {
        throw "Remote tag $tagName already exists."
    }

    $changes = @(git -c core.quotepath=false diff --name-status --no-renames "$base..$head")
    if (-not $changes) {
        throw "No changes found between origin/main and HEAD."
    }

    $treeEntries = [System.Collections.Generic.List[object]]::new()
    $uploaded = 0
    foreach ($line in $changes) {
        $parts = $line -split "`t", 2
        if ($parts.Count -ne 2) {
            throw "Cannot parse changed path."
        }
        $status = $parts[0]
        $path = $parts[1]

        if ($status -eq "D") {
            $treeEntries.Add([ordered]@{
                path = $path
                mode = "100644"
                type = "blob"
                sha = $null
            })
            continue
        }
        if ($status -notin @("A", "M")) {
            throw "Unsupported change status $status."
        }

        $lsTree = (git -c core.quotepath=false ls-tree $head -- $path)
        if ($lsTree -notmatch '^(\d+)\s+(\w+)\s+([0-9a-f]{40})\t') {
            throw "Cannot resolve Git object for changed path."
        }
        $mode = $Matches[1]
        $type = $Matches[2]
        $expectedBlob = $Matches[3]
        if ($type -ne "blob") {
            throw "Only blob changes are supported by this release publisher."
        }

        [byte[]]$blobBytes = Get-GitBlobBytes -ObjectId $expectedBlob
        $content = [Convert]::ToBase64String($blobBytes)
        $blob = Invoke-GitHubApi -Method POST -Uri "$apiRoot/git/blobs" -Body @{
            content = $content
            encoding = "base64"
        }
        if ($blob.sha -ne $expectedBlob) {
            throw "Uploaded blob SHA does not match local Git object."
        }
        $treeEntries.Add([ordered]@{
            path = $path
            mode = $mode
            type = $type
            sha = $blob.sha
        })
        $uploaded++
        if (($uploaded % 10) -eq 0) {
            Write-Output "Uploaded $uploaded changed blobs."
        }
    }

    $tree = Invoke-GitHubApi -Method POST -Uri "$apiRoot/git/trees" -Body @{
        base_tree = $remoteHead
        tree = $treeEntries
    }
    if ($tree.sha -ne $localTree) {
        throw "Remote tree $($tree.sha) does not match local tree $localTree."
    }
    Write-Output "Verified identical Git tree: $localTree"

    if (-not $Publish) {
        Write-Output "Dry run complete; refs were not changed."
        exit 0
    }

    $name = (git config user.name).Trim()
    $email = (git config user.email).Trim()
    if (-not $name) { $name = $owner }
    if (-not $email) { $email = "$owner@users.noreply.github.com" }
    $date = [DateTimeOffset]::Now.ToString("o")
    $identity = @{ name = $name; email = $email; date = $date }

    $commit = Invoke-GitHubApi -Method POST -Uri "$apiRoot/git/commits" -Body @{
        message = "release: replace desktop app with DiCAR LAB v1.7.0"
        tree = $tree.sha
        parents = @($remoteHead)
        author = $identity
        committer = $identity
    }
    $tag = Invoke-GitHubApi -Method POST -Uri "$apiRoot/git/tags" -Body @{
        tag = $tagName
        message = "DiCAR LAB v1.7.0"
        object = $commit.sha
        type = "commit"
        tagger = $identity
    }

    Invoke-GitHubApi -Method PATCH -Uri "$apiRoot/git/refs/heads/main" -Body @{
        sha = $commit.sha
        force = $false
    } | Out-Null
    Invoke-GitHubApi -Method POST -Uri "$apiRoot/git/refs" -Body @{
        ref = "refs/tags/$tagName"
        sha = $tag.sha
    } | Out-Null

    Write-Output "Published main commit: $($commit.sha)"
    Write-Output "Published tag: $tagName"
    Write-Output "Repository: https://github.com/$owner/$repo"
} finally {
    $token = $null
    $script:headers.Authorization = $null
}
