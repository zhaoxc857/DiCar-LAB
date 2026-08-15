import { Navigate, Route, Routes } from "react-router";
import { AppShell } from "../components/shell/AppShell";
import { DiagnosticsPage } from "../pages/DiagnosticsPage";
import { HomePage } from "../pages/HomePage";
import { LiveWorkbenchPage } from "../pages/LiveWorkbenchPage";
import { NotFoundPage } from "../pages/NotFoundPage";
import { RecordingsPage } from "../pages/RecordingsPage";

export function AppRoutes() {
  return <Routes><Route element={<AppShell />}><Route element={<HomePage />} index /><Route element={<DiagnosticsPage />} path="diagnostics" /><Route element={<LiveWorkbenchPage />} path="live" /><Route element={<Navigate replace to="/live" />} path="live/:vehicleId" /><Route element={<RecordingsPage />} path="records" /><Route element={<NotFoundPage />} path="*" /></Route></Routes>;
}
