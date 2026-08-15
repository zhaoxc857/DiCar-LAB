import { X } from "@phosphor-icons/react";
import * as Dialog from "@radix-ui/react-dialog";
import type { PropsWithChildren } from "react";
import { cn } from "../../lib/cn";

export type DrawerProps = PropsWithChildren<{
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  description: string;
  side?: "left" | "right";
}>;

export function Drawer({
  open,
  onOpenChange,
  title,
  description,
  side = "right",
  children,
}: DrawerProps) {
  return (
    <Dialog.Root onOpenChange={onOpenChange} open={open}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-40 bg-black/65" />
        <Dialog.Content
          className={cn(
            "fixed inset-y-0 z-50 flex w-[min(92vw,440px)] flex-col border-(--border) bg-(--surface-raised) shadow-2xl",
            side === "right" ? "right-0 border-l" : "left-0 border-r",
          )}
        >
          <header className="flex items-start gap-3 border-b border-(--border) p-4">
            <div className="min-w-0 flex-1">
              <Dialog.Title className="m-0 text-base font-semibold">{title}</Dialog.Title>
              <Dialog.Description className="m-0 mt-1 text-xs leading-5 text-(--text-muted)">
                {description}
              </Dialog.Description>
            </div>
            <Dialog.Close
              aria-label={`关闭${title}`}
              className="grid size-11 shrink-0 place-items-center rounded-[var(--radius-sm)] border border-transparent text-(--text-muted) hover:border-(--border) hover:text-(--text)"
            >
              <X aria-hidden="true" size={18} />
            </Dialog.Close>
          </header>
          <div className="min-h-0 flex-1 overflow-y-auto p-4">{children}</div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
