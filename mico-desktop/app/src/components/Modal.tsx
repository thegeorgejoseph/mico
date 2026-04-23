import { X } from "lucide-react";
import type { ReactNode } from "react";

interface ModalProps {
  children: ReactNode;
  onClose: () => void;
  open: boolean;
  subtitle: string;
  title: string;
}

export function Modal({ children, onClose, open, subtitle, title }: ModalProps) {
  if (!open) {
    return null;
  }
  return (
    <div className="modal-backdrop" role="presentation" onMouseDown={onClose}>
      <section className="modal" role="dialog" aria-modal="true" aria-label={title} onMouseDown={(event) => event.stopPropagation()}>
        <div className="modal__header">
          <div>
            <h2>{title}</h2>
            <p>{subtitle}</p>
          </div>
          <button onClick={onClose} type="button" aria-label="Close" title="Close">
            <X size={16} />
          </button>
        </div>
        {children}
      </section>
    </div>
  );
}
