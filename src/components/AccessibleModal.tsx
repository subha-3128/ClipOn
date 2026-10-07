import React, { useEffect, useRef } from "react";

export interface AccessibleModalProps {
  isOpen: boolean;
  onClose: () => void;
  title: string;
  titleId?: string;
  descriptionId?: string;
  overlayClassName?: string;
  dialogClassName?: string;
  children: React.ReactNode;
}

/**
 * Reusable accessible modal primitive implementing WAI-ARIA Dialog (Modal) pattern:
 * - role="dialog" and aria-modal="true"
 * - Keyboard Escape listener for dismissal
 * - Focus trapping (Tab and Shift+Tab wrap within dialog)
 * - Focus restoration (returns focus to previously active element upon close)
 * - Click outside overlay dismissal
 */
export function AccessibleModal({
  isOpen,
  onClose,
  titleId = "accessible-dialog-title",
  descriptionId,
  overlayClassName = "modal-overlay",
  dialogClassName,
  children,
}: AccessibleModalProps) {
  const dialogRef = useRef<HTMLDivElement>(null);
  const previousActiveElement = useRef<HTMLElement | null>(null);

  useEffect(() => {
    if (!isOpen) return;

    // Save previous active element for focus restoration upon close
    previousActiveElement.current =
      document.activeElement as HTMLElement | null;

    // Focus the first focusable element inside the modal
    const dialogEl = dialogRef.current;
    if (dialogEl) {
      const focusables = dialogEl.querySelectorAll<HTMLElement>(
        'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])'
      );
      if (focusables.length > 0) {
        // slight timeout ensures modal animation/rendering is ready
        setTimeout(() => focusables[0]?.focus(), 10);
      } else {
        dialogEl.focus();
      }
    }

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
        return;
      }

      // Trap focus inside modal
      if (e.key === "Tab" && dialogRef.current) {
        const focusables = dialogRef.current.querySelectorAll<HTMLElement>(
          'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])'
        );
        if (focusables.length === 0) return;

        const firstElement = focusables[0];
        const lastElement = focusables[focusables.length - 1];

        if (e.shiftKey) {
          if (document.activeElement === firstElement) {
            e.preventDefault();
            lastElement.focus();
          }
        } else {
          if (document.activeElement === lastElement) {
            e.preventDefault();
            firstElement.focus();
          }
        }
      }
    };

    window.addEventListener("keydown", handleKeyDown);

    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      // Restore focus to triggering button
      if (
        previousActiveElement.current &&
        typeof previousActiveElement.current.focus === "function"
      ) {
        previousActiveElement.current.focus();
      }
    };
  }, [isOpen, onClose]);

  if (!isOpen) return null;

  return (
    <div className={overlayClassName} onClick={onClose} role="presentation">
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={descriptionId}
        tabIndex={-1}
        className={dialogClassName}
        onClick={(e) => e.stopPropagation()}
        style={{ outline: "none" }}
      >
        {children}
      </div>
    </div>
  );
}
