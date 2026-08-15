import { render, screen } from "@testing-library/react";
import { AppProviders } from "../../app/providers";
import { MockBridge } from "../../bridge/mockBridge";
import { ChangeBar } from "./ChangeBar";

it("does not occupy the viewport when there are no dirty parameters", () => {
  render(<AppProviders bridge={new MockBridge()}><ChangeBar dirtyCount={0} onReview={() => undefined} /></AppProviders>);
  expect(screen.queryByText("0 项待固化")).not.toBeInTheDocument();
});
