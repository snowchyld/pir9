/**
 * Modal container for dynamic modals
 */

import { BaseComponent, customElement, html } from '../../core/component';
import { activeModal, closeModal } from '../../stores/app.store';

@customElement('modal-container')
export class ModalContainer extends BaseComponent {
  protected onInit(): void {
    this.watch(activeModal);

    // Close on escape key
    document.addEventListener('keydown', this.handleKeydown);
  }

  protected onDestroy(): void {
    document.removeEventListener('keydown', this.handleKeydown);
  }

  private handleKeydown = (event: KeyboardEvent): void => {
    if (event.key === 'Escape' && activeModal.value) {
      closeModal();
    }
  };

  protected template(): string {
    const modal = activeModal.value;

    if (!modal) {
      return '';
    }

    return html`
      <div class="modal-backdrop" onclick="this.closest('modal-container').handleBackdropClick(event)">
        <div class="modal-content" role="dialog" aria-modal="true">
          <slot name="modal-${modal.type}"></slot>
        </div>
      </div>

      <style>
        :host {
          display: contents;
        }

        .modal-backdrop {
          position: fixed;
          inset: 0;
          z-index: 100;
          display: flex;
          align-items: center;
          justify-content: center;
          padding: 1rem;
          background-color: var(--modal-backdrop);
          animation: fadeIn 0.15s ease-out;
        }

        .modal-content {
          position: relative;
          max-width: 90vw;
          max-height: 90vh;
          overflow: auto;
          background-color: var(--bg-modal);
          border-radius: 0.5rem;
          box-shadow: 0 10px 40px rgba(0, 0, 0, 0.4);
          animation: slideUp 0.2s ease-out;
        }

        @keyframes fadeIn {
          from { opacity: 0; }
          to { opacity: 1; }
        }

        @keyframes slideUp {
          from {
            opacity: 0;
            transform: translateY(20px);
          }
          to {
            opacity: 1;
            transform: translateY(0);
          }
        }
      </style>
    `;
  }

  handleBackdropClick(event: MouseEvent): void {
    if (event.target === event.currentTarget) {
      closeModal();
    }
  }
}
