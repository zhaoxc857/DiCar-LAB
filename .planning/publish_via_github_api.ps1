param(
    [switch]$Publish,
    [switch]$NoTag,
    [string]$TagName = "v1.8.0",
    [string]$CommitMessage = "release: DiCAR LAB v1.8.0 wireless flashing, threaded serial, scope capture",
    [string]$BaseCommit = "bb755e3"
)

$ErrorActionPreference = "Stop"
# Git outputs UTF-8; PowerShell 5.1 defaults to the ANSI code page (GBK on
# zh-CN hosts), which mojibakes non-ASCII paths (e.g. README_小白用户.txt)
# into the remote tree. Decode all native output as UTF-8.
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8
$owner = "zhaoxc857"
$repo = "DiCar-LAB"
$tagName = $TagName
# Canonical API root: the repos/{owner}/{repo} endpoint 301-redirects here
# (repository was renamed), and redirected requests drop the auth header.
$apiRoot = "https://api.github.com/repositories/1336759699"

function Invoke-GitHubApiRaw {
    param(
        [Parameter(Mandatory)] [ValidateSet("GET", "POST", "PATCH", "PUT")] [string]$Method,
        [Parameter(Mandatory)] [string]$Uri,
        [object]$Body
    )

    # Windows PowerShell 5.1 Invoke-RestMethod intermittently sends requests
    # without usable auth on this host, so the HTTP engine is curl, which is
    # verified to work against api.github.com from this machine.
    $hdrFile = [IO.Path]::GetTempFileName()
    $outFile = [IO.Path]::GetTempFileName()
    try {
        [IO.File]::WriteAllText($hdrFile, @(
            "Authorization: Bearer $script:token",
            "Accept: application/vnd.github+json",
            "X-GitHub-Api-Version: 2022-11-28",
            "User-Agent: DiCAR-LAB-release"
        ) -join "`n")

        $curlArgs = @("-sS", "-X", $Method, "-H", "@$hdrFile", "-o", $outFile, "-w", "%{http_code}")
        $bodyFile = $null
        if ($null -ne $Body) {
            $bodyFile = [IO.Path]::GetTempFileName()
            [IO.File]::WriteAllText($bodyFile, ($Body | ConvertTo-Json -Depth 12 -Compress))
            $curlArgs += @("-H", "Content-Type: application/json", "--data", "@$bodyFile")
        }
        $curlArgs += $Uri
        $status = (& curl.exe @curlArgs) -join ""
        if ($LASTEXITCODE -ne 0) {
            throw "curl exited $LASTEXITCODE for $Method $Uri."
        }
        $raw = [IO.File]::ReadAllText($outFile)
        return @{ status = $status; body = $raw }
    } finally {
        Remove-Item -LiteralPath $hdrFile -ErrorAction SilentlyContinue
        Remove-Item -LiteralPath $outFile -ErrorAction SilentlyContinue
        if ($bodyFile) { Remove-Item -LiteralPath $bodyFile -ErrorAction SilentlyContinue }
    }
}

function Invoke-GitHubApi {
    param(
        [Parameter(Mandatory)] [ValidateSet("GET", "POST", "PATCH", "PUT")] [string]$Method,
        [Parameter(Mandatory)] [string]$Uri,
        [object]$Body
    )

    $result = Invoke-GitHubApiRaw -Method $Method -Uri $Uri -Body $Body
    if ($result.status -notin @("200", "201", "204")) {
        throw "GitHub API $($result.status) for $Method ${Uri}: $($result.body)"
    }
    if ($result.status -eq "204" -or -not $result.body) {
        return $null
    }
    return $result.body | ConvertFrom-Json
}

function Test-GitHubResource {
    param([Parameter(Mandatory)] [string]$Uri)
    $result = Invoke-GitHubApiRaw -Method GET -Uri $Uri
    return ($result.status -eq "200")
}

function Get-GitBlobBytes {
    param([Parameter(Mandatory)] [string]$ObjectId)

    $startInfo = [Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = (Get-Command git).Source
    $startInfo.Arguments = "cat-file blob $ObjectId"
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
$script:token = $token
$credentialLines = $null

try {
    $root = (git rev-parse --show-toplevel).Trim()
    $head = (git rev-parse HEAD).Trim()
    $base = (git rev-parse $BaseCommit).Trim()
    $localTree = (git rev-parse "$head`^{tree}").Trim()
    $remoteRef = Invoke-GitHubApi -Method GET -Uri "$apiRoot/git/ref/heads/main"
    $remoteHead = $remoteRef.object.sha

    $remoteTree = (Invoke-GitHubApi -Method GET -Uri "$apiRoot/git/commits/$remoteHead").tree.sha
    $baseTree = (git rev-parse "$base`^{tree}").Trim()
    if ($remoteTree -ne $baseTree) {
        throw "Remote main tree $remoteTree does not match base $base tree $baseTree."
    }
    if (-not $NoTag -and (Test-GitHubResource -Uri "$apiRoot/git/ref/tags/$tagName")) {
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
        message = $CommitMessage
        tree = $tree.sha
        parents = @($remoteHead)
        author = $identity
        committer = $identity
    }
    if (-not $NoTag) {
        $tag = Invoke-GitHubApi -Method POST -Uri "$apiRoot/git/tags" -Body @{
            tag = $tagName
            message = "DiCAR LAB $tagName"
            object = $commit.sha
            type = "commit"
            tagger = $identity
        }
    }

    Invoke-GitHubApi -Method PATCH -Uri "$apiRoot/git/refs/heads/main" -Body @{
        sha = $commit.sha
        force = $false
    } | Out-Null
    if (-not $NoTag) {
        Invoke-GitHubApi -Method POST -Uri "$apiRoot/git/refs" -Body @{
            ref = "refs/tags/$tagName"
            sha = $tag.sha
        } | Out-Null
        Write-Output "Published tag: $tagName"
    }

    Write-Output "Published main commit: $($commit.sha)"
    Write-Output "Repository: https://github.com/$owner/$repo"
} finally {
    $script:token = $null
}
