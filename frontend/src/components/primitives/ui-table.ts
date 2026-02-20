/**
 * Data table component with sorting
 */

import {
  attribute,
  BaseComponent,
  customElement,
  escapeHtml,
  html,
  safeHtml,
} from '../../core/component';

export interface TableColumn<T = unknown> {
  key: string;
  label: string;
  sortable?: boolean;
  width?: string;
  align?: 'left' | 'center' | 'right';
  render?: (value: unknown, row: T, index: number) => string;
}

@customElement('ui-table')
export class UITable extends BaseComponent {
  @attribute() sortKey = '';
  @attribute() sortDirection: 'asc' | 'desc' = 'asc';
  @attribute({ type: 'boolean' }) loading = false;
  @attribute() emptyMessage = 'No data available';

  private _columns: TableColumn[] = [];
  private _data: unknown[] = [];

  get columns(): TableColumn[] {
    return this._columns;
  }

  set columns(value: TableColumn[]) {
    this._columns = value;
    if (this._isConnected) {
      this.requestUpdate();
    }
  }

  get data(): unknown[] {
    return this._data;
  }

  set data(value: unknown[]) {
    this._data = value;
    if (this._isConnected) {
      this.requestUpdate();
    }
  }

  protected template(): string {
    return html`
      <div class="table-container">
        <table class="table">
          <thead>
            <tr>
              ${this._columns.map((col) => this.renderHeader(col)).join('')}
            </tr>
          </thead>
          <tbody>
            ${
              this.loading
                ? this.renderLoading()
                : this._data.length === 0
                  ? this.renderEmpty()
                  : this._data.map((row, idx) => this.renderRow(row, idx)).join('')
            }
          </tbody>
        </table>
      </div>

      <style>
        :host {
          display: block;
        }

        .table-container {
          overflow-x: auto;
        }

        .table {
          width: 100%;
          border-collapse: collapse;
          font-size: 0.875rem;
        }

        .table th,
        .table td {
          padding: 0.75rem;
          text-align: left;
          border-bottom: 1px solid var(--border-color);
        }

        .table th {
          font-weight: 600;
          color: var(--text-color-muted);
          white-space: nowrap;
          background-color: var(--bg-card-alt);
        }

        .table th.sortable {
          cursor: pointer;
          user-select: none;
        }

        .table th.sortable:hover {
          color: var(--text-color);
        }

        .table th.sorted {
          color: var(--color-primary);
        }

        .th-content {
          display: flex;
          align-items: center;
          gap: 0.25rem;
        }

        .sort-icon {
          width: 14px;
          height: 14px;
          opacity: 0.5;
        }

        .sorted .sort-icon {
          opacity: 1;
        }

        .sort-icon.desc {
          transform: rotate(180deg);
        }

        .table tbody tr:hover td {
          background-color: var(--bg-table-row-hover);
        }

        .align-left { text-align: left; }
        .align-center { text-align: center; }
        .align-right { text-align: right; }

        .loading-row td,
        .empty-row td {
          text-align: center;
          padding: 2rem;
          color: var(--text-color-muted);
        }

        .loading-spinner {
          display: inline-block;
          width: 20px;
          height: 20px;
          border: 2px solid var(--border-color);
          border-top-color: var(--color-primary);
          border-radius: 50%;
          animation: spin 0.8s linear infinite;
        }

        @keyframes spin {
          to { transform: rotate(360deg); }
        }
      </style>
    `;
  }

  private renderHeader(col: TableColumn): string {
    const isSorted = this.sortKey === col.key;
    const classes = this.cx(
      col.sortable && 'sortable',
      isSorted && 'sorted',
      col.align && `align-${col.align}`,
    );

    const sortIcon = col.sortable
      ? `
      <svg class="sort-icon ${isSorted && this.sortDirection === 'desc' ? 'desc' : ''}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <polyline points="18 15 12 9 6 15"></polyline>
      </svg>
    `
      : '';

    return html`
      <th
        class="${classes}"
        style="${col.width ? `width: ${col.width}` : ''}"
        ${col.sortable ? `onclick="this.closest('ui-table').handleSort('${col.key}')"` : ''}
      >
        <span class="th-content">
          ${escapeHtml(col.label)}
          ${safeHtml(sortIcon)}
        </span>
      </th>
    `;
  }

  private renderRow(row: unknown, index: number): string {
    const rowObj = row as Record<string, unknown>;

    return html`
      <tr onclick="this.closest('ui-table').handleRowClick(${index})">
        ${this._columns
          .map((col) => {
            const value = rowObj[col.key];
            const content = col.render
              ? col.render(value, row, index)
              : escapeHtml(String(value ?? ''));

            return `<td class="${col.align ? `align-${col.align}` : ''}">${content}</td>`;
          })
          .join('')}
      </tr>
    `;
  }

  private renderLoading(): string {
    return html`
      <tr class="loading-row">
        <td colspan="${this._columns.length}">
          <span class="loading-spinner"></span>
        </td>
      </tr>
    `;
  }

  private renderEmpty(): string {
    return html`
      <tr class="empty-row">
        <td colspan="${this._columns.length}">
          ${escapeHtml(this.emptyMessage)}
        </td>
      </tr>
    `;
  }

  handleSort(key: string): void {
    if (this.sortKey === key) {
      this.sortDirection = this.sortDirection === 'asc' ? 'desc' : 'asc';
    } else {
      this.sortKey = key;
      this.sortDirection = 'asc';
    }
    this.emit('sort', { key: this.sortKey, direction: this.sortDirection });
  }

  handleRowClick(index: number): void {
    this.emit('row-click', { index, row: this._data[index] });
  }
}
