import { Link } from "react-router";

export function NotFoundPage() {
  return <main className="mx-auto max-w-xl px-4 py-16 text-center" id="main-content"><p className="font-mono text-(--interactive)">404</p><h1>页面不存在</h1><Link className="text-sm text-(--interactive)" to="/">返回工作区</Link></main>;
}
