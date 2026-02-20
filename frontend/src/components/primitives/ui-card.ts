/**
 * Card container component
 */

import { attribute, BaseComponent, customElement, html } from '../../core/component';

@customElement('ui-card')
export class UICard extends BaseComponent {
  @attribute({ type: 'boolean' }) hoverable = false;
  @attribute({ type: 'boolean' }) clickable = false;

  protected template(): string {
    const classes = this.cx('card', {
      hoverable: this.hoverable,
      clickable: this.clickable,
    });

    return html`
      <div class="${classes}">
        <slot name="header"></slot>
        <div class="card-body">
          <slot></slot>
        </div>
        <slot name="footer"></slot>
      </div>

      <style>
        :host {
          display: block;
        }

        .card {
          background-color: var(--bg-card);
          border-radius: 0.375rem;
          box-shadow: 0 1px 3px var(--shadow-card);
          overflow: hidden;
        }

        .card.hoverable {
          transition: transform 0.15s ease, box-shadow 0.15s ease;
        }

        .card.hoverable:hover {
          transform: translateY(-2px);
          box-shadow: 0 4px 12px var(--shadow-card);
        }

        .card.clickable {
          cursor: pointer;
        }

        .card-body {
          padding: 1rem;
        }

        ::slotted([slot="header"]) {
          padding: 0.75rem 1rem;
          border-bottom: 1px solid var(--border-color);
          font-weight: 600;
        }

        ::slotted([slot="footer"]) {
          padding: 0.75rem 1rem;
          border-top: 1px solid var(--border-color);
          background-color: var(--bg-card-alt);
        }
      </style>
    `;
  }
}
