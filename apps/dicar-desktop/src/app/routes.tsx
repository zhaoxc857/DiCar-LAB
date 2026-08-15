import { Route, Routes } from "react-router";
import { AppShell } from "../components/shell/AppShell";
import { ComingSoonPage } from "../pages/ComingSoonPage";
import { DiagnosticsPage } from "../pages/DiagnosticsPage";
import { HomePage } from "../pages/HomePage";
import { LiveWorkbenchPage } from "../pages/LiveWorkbenchPage";
import { NotFoundPage } from "../pages/NotFoundPage";
import { RecordingsPage } from "../pages/RecordingsPage";

export function AppRoutes() {
  return <Routes><Route element={<AppShell />}><Route element={<HomePage />} index /><Route element={<DiagnosticsPage />} path="diagnostics" /><Route element={<LiveWorkbenchPage />} path="live/:vehicleId" /><Route element={<RecordingsPage />} path="records" /><Route element={<ComingSoonPage title="参数方案库" scope="参数方案比较、评审和版本追踪将在" />} path="parameter-sets" /><Route element={<NotFoundPage />} path="*" /></Route></Routes>;
}
