import { fireEvent, render, screen } from "@testing-library/react";
import { useVehicleProfileStore } from "../../stores/vehicleProfileStore";
import { VehicleProfileManager } from "./VehicleProfileManager";

const USER_YAML = "schema_version: 1\nvehicle: { id: user-car, display_name: 用户车, type: 测试, order: 50 }\n";
const UPDATED_USER_YAML = "schema_version: 1\nvehicle: { id: user-car, display_name: 更新用户车, type: 测试, order: 50 }\n";

beforeEach(() => useVehicleProfileStore.getState().reset());

it("imports through the real file input and requires confirmation before replacement", async () => {
  render(<VehicleProfileManager onClose={() => undefined} open />);
  const input = screen.getByLabelText("导入车型 YAML");
  fireEvent.change(input, { target: { files: [new File([USER_YAML], "user-car.yaml", { type: "application/yaml" })] } });
  expect(await screen.findByText("已导入 用户车")).toBeInTheDocument();
  fireEvent.change(input, { target: { files: [new File([UPDATED_USER_YAML], "user-car.yaml", { type: "application/yaml" })] } });
  expect(await screen.findByRole("button", { name: "确认替换 更新用户车" })).toBeInTheDocument();
});

it("shows built-in source and removes user profiles only", async () => {
  useVehicleProfileStore.getState().importProfile(USER_YAML, false);
  render(<VehicleProfileManager onClose={() => undefined} open />);
  expect(screen.getByText("内置", { exact: true })).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "移除 用户车" }));
  expect(screen.getByText("用户车")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "确认移除 用户车" }));
  expect(screen.queryByText("用户车")).not.toBeInTheDocument();
});
