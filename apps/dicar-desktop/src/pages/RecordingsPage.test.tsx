import { fireEvent, render, screen } from "@testing-library/react";
import { App } from "../app/App";
import { AppProviders } from "../app/providers";
import { seededRecordingController } from "../test/seededRecordingController";

it("serves the completed recording library at /records", async () => {
  window.history.pushState({}, "", "/records");
  const { bridge, controller } = await seededRecordingController();
  render(
    <AppProviders bridge={bridge} recordingController={controller}>
      <App />
    </AppProviders>,
  );

  expect(await screen.findByRole("heading", { name: "波形记录" })).toBeInTheDocument();
  expect(await screen.findByText("最新记录")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "回放 最新记录" }));
  expect(await screen.findByRole("dialog", { name: "回放 · 最新记录" })).toBeInTheDocument();
});
