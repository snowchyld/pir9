/**
 * Calendar page showing upcoming episodes
 */

import { BaseComponent, customElement, html, escapeHtml, safeHtml } from '../../core/component';
import { createQuery } from '../../core/query';
import { http, type CalendarEvent } from '../../core/http';
import { navigate } from '../../router';
import { signal } from '../../core/reactive';

type CalendarView = 'month' | 'week' | 'agenda';

@customElement('calendar-page')
export class CalendarPage extends BaseComponent {
  private currentDate = signal(new Date());
  private view = signal<CalendarView>('week');

  private get dateRange() {
    const date = this.currentDate.value;
    const view = this.view.value;

    if (view === 'month') {
      const start = new Date(date.getFullYear(), date.getMonth(), 1);
      const end = new Date(date.getFullYear(), date.getMonth() + 1, 0);
      return { start, end };
    } else if (view === 'week') {
      const start = new Date(date);
      start.setDate(date.getDate() - date.getDay());
      const end = new Date(start);
      end.setDate(start.getDate() + 6);
      return { start, end };
    } else {
      const start = new Date(date);
      const end = new Date(date);
      end.setDate(start.getDate() + 30);
      return { start, end };
    }
  }

  private calendarQuery = createQuery({
    queryKey: ['/calendar', this.dateRange.start.toISOString(), this.dateRange.end.toISOString()],
    queryFn: () => {
      const { start, end } = this.dateRange;
      return http.get<CalendarEvent[]>('/calendar', {
        params: {
          start: start.toISOString(),
          end: end.toISOString(),
        },
      });
    },
  });

  protected onInit(): void {
    this.watch(this.currentDate);
    this.watch(this.view);
    this.watch(this.calendarQuery.data);
    this.watch(this.calendarQuery.isLoading);
  }

  protected template(): string {
    const events = this.calendarQuery.data.value ?? [];
    const isLoading = this.calendarQuery.isLoading.value;
    const view = this.view.value;
    const date = this.currentDate.value;

    return html`
      <div class="calendar-page">
        <!-- Toolbar -->
        <div class="toolbar">
          <div class="toolbar-left">
            <h1 class="page-title">Calendar</h1>
          </div>

          <div class="toolbar-center">
            <button class="nav-btn" onclick="this.closest('calendar-page').navigatePrev()">
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <polyline points="15 18 9 12 15 6"></polyline>
              </svg>
            </button>
            <span class="current-period">${this.formatPeriod(date, view)}</span>
            <button class="nav-btn" onclick="this.closest('calendar-page').navigateNext()">
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <polyline points="9 18 15 12 9 6"></polyline>
              </svg>
            </button>
            <button class="today-btn" onclick="this.closest('calendar-page').goToToday()">Today</button>
          </div>

          <div class="toolbar-right">
            <div class="view-modes">
              ${this.renderViewButton('week', 'Week')}
              ${this.renderViewButton('month', 'Month')}
              ${this.renderViewButton('agenda', 'Agenda')}
            </div>
          </div>
        </div>

        <!-- Content -->
        <div class="calendar-content">
          ${isLoading ? this.renderLoading() : ''}
          ${!isLoading && view === 'week' ? this.renderWeekView(events) : ''}
          ${!isLoading && view === 'month' ? this.renderMonthView(events) : ''}
          ${!isLoading && view === 'agenda' ? this.renderAgendaView(events) : ''}
        </div>

        <!-- Legend -->
        <div class="calendar-legend">
          <div class="legend-item">
            <span class="legend-dot unaired"></span>
            <span>Unaired</span>
          </div>
          <div class="legend-item">
            <span class="legend-dot on-air"></span>
            <span>On Air</span>
          </div>
          <div class="legend-item">
            <span class="legend-dot downloaded"></span>
            <span>Downloaded</span>
          </div>
          <div class="legend-item">
            <span class="legend-dot missing"></span>
            <span>Missing</span>
          </div>
        </div>
      </div>

      <style>
        .calendar-page {
          display: flex;
          flex-direction: column;
          gap: 1rem;
        }

        .toolbar {
          display: flex;
          align-items: center;
          justify-content: space-between;
          flex-wrap: wrap;
          gap: 1rem;
        }

        .toolbar-left, .toolbar-center, .toolbar-right {
          display: flex;
          align-items: center;
          gap: 0.5rem;
        }

        .page-title {
          font-size: 1.5rem;
          font-weight: 600;
          margin: 0;
        }

        .nav-btn {
          display: flex;
          align-items: center;
          justify-content: center;
          padding: 0.375rem;
          background: transparent;
          border: 1px solid var(--border-color);
          border-radius: 0.25rem;
          color: var(--text-color);
          cursor: pointer;
        }

        .nav-btn:hover {
          background-color: var(--bg-input-hover);
        }

        .current-period {
          min-width: 150px;
          text-align: center;
          font-weight: 500;
        }

        .today-btn {
          padding: 0.375rem 0.75rem;
          background-color: var(--btn-default-bg);
          border: 1px solid var(--btn-default-border);
          border-radius: 0.25rem;
          color: var(--text-color);
          font-size: 0.875rem;
          cursor: pointer;
        }

        .today-btn:hover {
          background-color: var(--btn-default-bg-hover);
        }

        .view-modes {
          display: flex;
          border: 1px solid var(--border-input);
          border-radius: 0.25rem;
          overflow: hidden;
        }

        .view-btn {
          padding: 0.375rem 0.75rem;
          background-color: var(--bg-input);
          color: var(--text-color-muted);
          border: none;
          font-size: 0.875rem;
          cursor: pointer;
        }

        .view-btn:not(:last-child) {
          border-right: 1px solid var(--border-input);
        }

        .view-btn.active {
          background-color: var(--color-primary);
          color: var(--color-white);
        }

        .view-btn:hover:not(.active) {
          background-color: var(--bg-input-hover);
        }

        /* Loading */
        .loading-container {
          display: flex;
          justify-content: center;
          padding: 4rem;
        }

        .loading-spinner {
          width: 32px;
          height: 32px;
          border: 3px solid var(--border-color);
          border-top-color: var(--color-primary);
          border-radius: 50%;
          animation: spin 0.8s linear infinite;
        }

        @keyframes spin {
          to { transform: rotate(360deg); }
        }

        /* Week View */
        .week-grid {
          display: grid;
          grid-template-columns: repeat(7, 1fr);
          gap: 1px;
          background-color: var(--border-color);
          border: 1px solid var(--border-color);
          border-radius: 0.375rem;
          overflow: hidden;
        }

        .day-header {
          padding: 0.5rem;
          background-color: var(--bg-card-alt);
          text-align: center;
          font-size: 0.75rem;
          font-weight: 600;
          color: var(--text-color-muted);
          text-transform: uppercase;
        }

        .day-cell {
          min-height: 120px;
          padding: 0.5rem;
          background-color: var(--bg-calendar);
        }

        .day-cell.today {
          background-color: var(--bg-calendar-today);
        }

        .day-cell.other-month {
          opacity: 0.5;
        }

        .day-number {
          font-size: 0.875rem;
          font-weight: 500;
          margin-bottom: 0.5rem;
        }

        .event-card {
          padding: 0.25rem 0.5rem;
          margin-bottom: 0.25rem;
          border-radius: 0.25rem;
          font-size: 0.75rem;
          cursor: pointer;
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
        }

        .event-card.unaired {
          background-color: var(--color-primary);
          color: var(--color-white);
        }

        .event-card.downloaded {
          background-color: var(--color-success);
          color: var(--color-white);
        }

        .event-card.missing {
          background-color: var(--color-danger);
          color: var(--color-white);
        }

        .event-card:hover {
          opacity: 0.9;
        }

        /* Agenda View */
        .agenda-list {
          display: flex;
          flex-direction: column;
          gap: 0.5rem;
        }

        .agenda-date {
          font-weight: 600;
          color: var(--text-color-muted);
          padding: 0.5rem 0;
          border-bottom: 1px solid var(--border-color);
          margin-top: 1rem;
        }

        .agenda-date:first-child {
          margin-top: 0;
        }

        .agenda-item {
          display: flex;
          align-items: center;
          gap: 1rem;
          padding: 0.75rem;
          background-color: var(--bg-card);
          border-radius: 0.375rem;
          cursor: pointer;
        }

        .agenda-item:hover {
          background-color: var(--bg-card-alt);
        }

        .agenda-time {
          font-size: 0.875rem;
          color: var(--text-color-muted);
          min-width: 60px;
        }

        .agenda-poster {
          width: 40px;
          height: 60px;
          border-radius: 0.25rem;
          object-fit: cover;
          background-color: var(--bg-card-center);
        }

        .agenda-info {
          flex: 1;
          min-width: 0;
        }

        .agenda-series {
          font-weight: 500;
          white-space: nowrap;
          overflow: hidden;
          text-overflow: ellipsis;
        }

        .agenda-episode {
          font-size: 0.875rem;
          color: var(--text-color-muted);
        }

        .agenda-status {
          width: 8px;
          height: 8px;
          border-radius: 50%;
          flex-shrink: 0;
        }

        .agenda-status.downloaded { background-color: var(--color-success); }
        .agenda-status.missing { background-color: var(--color-danger); }
        .agenda-status.unaired { background-color: var(--color-primary); }

        /* Legend */
        .calendar-legend {
          display: flex;
          gap: 1.5rem;
          padding: 0.75rem;
          background-color: var(--bg-card);
          border-radius: 0.375rem;
          font-size: 0.875rem;
        }

        .legend-item {
          display: flex;
          align-items: center;
          gap: 0.5rem;
        }

        .legend-dot {
          width: 12px;
          height: 12px;
          border-radius: 0.25rem;
        }

        .legend-dot.unaired { background-color: var(--color-primary); }
        .legend-dot.on-air { background-color: var(--color-purple); }
        .legend-dot.downloaded { background-color: var(--color-success); }
        .legend-dot.missing { background-color: var(--color-danger); }

        .empty-message {
          text-align: center;
          padding: 2rem;
          color: var(--text-color-muted);
        }
      </style>
    `;
  }

  private renderViewButton(view: CalendarView, label: string): string {
    const isActive = this.view.value === view;
    return html`
      <button
        class="view-btn ${isActive ? 'active' : ''}"
        onclick="this.closest('calendar-page').setView('${view}')"
      >
        ${label}
      </button>
    `;
  }

  private renderLoading(): string {
    return html`
      <div class="loading-container">
        <div class="loading-spinner"></div>
      </div>
    `;
  }

  private renderWeekView(events: CalendarEvent[]): string {
    const { start } = this.dateRange;
    const days = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
    const today = new Date();
    today.setHours(0, 0, 0, 0);

    const eventsByDate = this.groupEventsByDate(events);

    let cells = '';
    for (let i = 0; i < 7; i++) {
      const date = new Date(start);
      date.setDate(start.getDate() + i);
      const dateStr = date.toISOString().split('T')[0];
      const isToday = date.getTime() === today.getTime();
      const dayEvents = eventsByDate.get(dateStr) ?? [];

      cells += html`
        <div class="day-cell ${isToday ? 'today' : ''}">
          <div class="day-number">${date.getDate()}</div>
          ${dayEvents.map((e) => this.renderEventCard(e)).join('')}
        </div>
      `;
    }

    return html`
      <div class="week-grid">
        ${days.map((d) => `<div class="day-header">${d}</div>`).join('')}
        ${cells}
      </div>
    `;
  }

  private renderMonthView(events: CalendarEvent[]): string {
    const date = this.currentDate.value;
    const firstDay = new Date(date.getFullYear(), date.getMonth(), 1);
    const lastDay = new Date(date.getFullYear(), date.getMonth() + 1, 0);
    const startDay = firstDay.getDay();
    const daysInMonth = lastDay.getDate();
    const today = new Date();
    today.setHours(0, 0, 0, 0);

    const days = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
    const eventsByDate = this.groupEventsByDate(events);

    let cells = '';
    const totalCells = Math.ceil((startDay + daysInMonth) / 7) * 7;

    for (let i = 0; i < totalCells; i++) {
      const dayNum = i - startDay + 1;
      const cellDate = new Date(date.getFullYear(), date.getMonth(), dayNum);
      const dateStr = cellDate.toISOString().split('T')[0];
      const isToday = cellDate.getTime() === today.getTime();
      const isOtherMonth = dayNum < 1 || dayNum > daysInMonth;
      const dayEvents = eventsByDate.get(dateStr) ?? [];

      cells += html`
        <div class="day-cell ${isToday ? 'today' : ''} ${isOtherMonth ? 'other-month' : ''}">
          <div class="day-number">${cellDate.getDate()}</div>
          ${dayEvents.slice(0, 3).map((e) => this.renderEventCard(e)).join('')}
          ${dayEvents.length > 3 ? `<div class="event-card">+${dayEvents.length - 3} more</div>` : ''}
        </div>
      `;
    }

    return html`
      <div class="week-grid">
        ${days.map((d) => `<div class="day-header">${d}</div>`).join('')}
        ${cells}
      </div>
    `;
  }

  private renderAgendaView(events: CalendarEvent[]): string {
    if (events.length === 0) {
      return html`<div class="empty-message">No upcoming episodes in this period</div>`;
    }

    const sortedEvents = [...events].sort((a, b) => {
      const aDate = a.airDateUtc ?? a.airDate ?? '';
      const bDate = b.airDateUtc ?? b.airDate ?? '';
      return aDate.localeCompare(bDate);
    });

    const groupedByDate = new Map<string, CalendarEvent[]>();
    for (const event of sortedEvents) {
      const dateStr = event.airDate ?? event.airDateUtc?.split('T')[0] ?? '';
      if (!groupedByDate.has(dateStr)) {
        groupedByDate.set(dateStr, []);
      }
      groupedByDate.get(dateStr)!.push(event);
    }

    let content = '';
    for (const [dateStr, dateEvents] of groupedByDate) {
      const date = new Date(dateStr);
      content += html`
        <div class="agenda-date">${this.formatDate(date)}</div>
        ${dateEvents.map((e) => this.renderAgendaItem(e)).join('')}
      `;
    }

    return html`<div class="agenda-list">${content}</div>`;
  }

  private renderEventCard(event: CalendarEvent): string {
    // Skip events with missing series data
    if (!event.series) {
      return '';
    }
    const status = this.getEventStatus(event);
    return html`
      <div
        class="event-card ${status}"
        onclick="this.closest('calendar-page').handleEventClick('${escapeHtml(event.series.titleSlug)}')"
        title="${escapeHtml(event.series.title)} - ${escapeHtml(event.title)}"
      >
        ${escapeHtml(event.series.title)}
      </div>
    `;
  }

  private renderAgendaItem(event: CalendarEvent): string {
    // Skip events with missing series data
    if (!event.series) {
      return '';
    }
    const status = this.getEventStatus(event);
    const poster = event.series.images?.find((i) => i.coverType === 'poster');
    const time = event.airDateUtc
      ? new Date(event.airDateUtc).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
      : '';

    return html`
      <div
        class="agenda-item"
        onclick="this.closest('calendar-page').handleEventClick('${escapeHtml(event.series.titleSlug)}')"
      >
        <span class="agenda-time">${time}</span>
        ${poster
          ? `<img class="agenda-poster" src="${escapeHtml(poster.url)}" alt="" />`
          : '<div class="agenda-poster"></div>'
        }
        <div class="agenda-info">
          <div class="agenda-series">${escapeHtml(event.series.title)}</div>
          <div class="agenda-episode">
            S${String(event.seasonNumber).padStart(2, '0')}E${String(event.episodeNumber).padStart(2, '0')} - ${escapeHtml(event.title)}
          </div>
        </div>
        <span class="agenda-status ${status}"></span>
      </div>
    `;
  }

  private groupEventsByDate(events: CalendarEvent[]): Map<string, CalendarEvent[]> {
    const map = new Map<string, CalendarEvent[]>();
    for (const event of events) {
      const dateStr = event.airDate ?? event.airDateUtc?.split('T')[0] ?? '';
      if (!map.has(dateStr)) {
        map.set(dateStr, []);
      }
      map.get(dateStr)!.push(event);
    }
    return map;
  }

  private getEventStatus(event: CalendarEvent): string {
    const now = new Date();
    const airDate = event.airDateUtc ? new Date(event.airDateUtc) : null;

    if (event.hasFile) {
      return 'downloaded';
    } else if (airDate && airDate > now) {
      return 'unaired';
    } else {
      return 'missing';
    }
  }

  private formatPeriod(date: Date, view: CalendarView): string {
    const options: Intl.DateTimeFormatOptions = { month: 'long', year: 'numeric' };
    if (view === 'week') {
      const { start, end } = this.dateRange;
      return `${start.toLocaleDateString(undefined, { month: 'short', day: 'numeric' })} - ${end.toLocaleDateString(undefined, { month: 'short', day: 'numeric', year: 'numeric' })}`;
    }
    return date.toLocaleDateString(undefined, options);
  }

  private formatDate(date: Date): string {
    const today = new Date();
    today.setHours(0, 0, 0, 0);
    const tomorrow = new Date(today);
    tomorrow.setDate(tomorrow.getDate() + 1);

    if (date.getTime() === today.getTime()) {
      return 'Today';
    } else if (date.getTime() === tomorrow.getTime()) {
      return 'Tomorrow';
    }
    return date.toLocaleDateString(undefined, { weekday: 'long', month: 'long', day: 'numeric' });
  }

  setView(view: CalendarView): void {
    this.view.set(view);
    this.calendarQuery.refetch();
  }

  navigatePrev(): void {
    const date = this.currentDate.value;
    const view = this.view.value;
    const newDate = new Date(date);

    if (view === 'month') {
      newDate.setMonth(date.getMonth() - 1);
    } else {
      newDate.setDate(date.getDate() - 7);
    }

    this.currentDate.set(newDate);
    this.calendarQuery.refetch();
  }

  navigateNext(): void {
    const date = this.currentDate.value;
    const view = this.view.value;
    const newDate = new Date(date);

    if (view === 'month') {
      newDate.setMonth(date.getMonth() + 1);
    } else {
      newDate.setDate(date.getDate() + 7);
    }

    this.currentDate.set(newDate);
    this.calendarQuery.refetch();
  }

  goToToday(): void {
    this.currentDate.set(new Date());
    this.calendarQuery.refetch();
  }

  handleEventClick(titleSlug: string): void {
    navigate(`/series/${titleSlug}`);
  }
}
