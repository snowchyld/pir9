/**
 * UI Settings page
 */

import { BaseComponent, customElement, html } from '../../core/component';
import { http } from '../../core/http';
import { createQuery } from '../../core/query';
import { showSuccess } from '../../stores/app.store';
import { setTheme, type Theme, themePreference } from '../../stores/theme.store';

interface UIConfig {
  firstDayOfWeek: number;
  calendarWeekColumnHeader: string;
  shortDateFormat: string;
  longDateFormat: string;
  timeFormat: string;
  showRelativeDates: boolean;
  enableColorImpairedMode: boolean;
  uiLanguage: number;
  theme: string;
}

@customElement('ui-settings')
export class UISettings extends BaseComponent {
  private uiQuery = createQuery({
    queryKey: ['/config/ui'],
    queryFn: () => http.get<UIConfig>('/config/ui'),
  });

  protected onInit(): void {
    this.watch(this.uiQuery.data);
    this.watch(this.uiQuery.isLoading);
    this.watch(themePreference);
  }

  protected template(): string {
    const ui = this.uiQuery.data.value;
    const isLoading = this.uiQuery.isLoading.value;
    const currentTheme = themePreference.value;

    if (isLoading) {
      return html`
        <div class="loading-container">
          <div class="loading-spinner"></div>
        </div>
      `;
    }

    return html`
      <div class="settings-section">
        <h2 class="section-title">Calendar</h2>

        <div class="form-group">
          <label class="form-label">First Day of Week</label>
          <select class="form-select">
            <option value="0" ${ui?.firstDayOfWeek === 0 ? 'selected' : ''}>Sunday</option>
            <option value="1" ${ui?.firstDayOfWeek === 1 ? 'selected' : ''}>Monday</option>
          </select>
        </div>

        <div class="form-group">
          <label class="form-label">Week Column Header</label>
          <select class="form-select">
            <option value="ddd M/D">Day of Week and Date</option>
            <option value="ddd">Day of Week</option>
            <option value="M/D">Date</option>
          </select>
        </div>
      </div>

      <div class="settings-section">
        <h2 class="section-title">Dates</h2>

        <div class="form-group">
          <label class="form-label">Short Date Format</label>
          <select class="form-select">
            <option value="MMM D YYYY">Mar 25 2024</option>
            <option value="DD MMM YYYY">25 Mar 2024</option>
            <option value="MM/DD/YYYY">03/25/2024</option>
            <option value="DD/MM/YYYY">25/03/2024</option>
            <option value="YYYY-MM-DD">2024-03-25</option>
          </select>
        </div>

        <div class="form-group">
          <label class="form-label">Long Date Format</label>
          <select class="form-select">
            <option value="dddd, MMMM D YYYY">Tuesday, March 25 2024</option>
            <option value="dddd, D MMMM YYYY">Tuesday, 25 March 2024</option>
          </select>
        </div>

        <div class="form-group">
          <label class="form-label">Time Format</label>
          <select class="form-select">
            <option value="h:mm a">5:00 PM</option>
            <option value="HH:mm">17:00</option>
          </select>
        </div>

        <div class="form-group">
          <label class="checkbox-label">
            <input
              type="checkbox"
              ${ui?.showRelativeDates ? 'checked' : ''}
            />
            <span>Show Relative Dates</span>
          </label>
          <p class="form-hint">Show relative (Today/Yesterday) instead of absolute dates</p>
        </div>
      </div>

      <div class="settings-section">
        <h2 class="section-title">Style</h2>

        <div class="form-group">
          <label class="form-label">Theme</label>
          <select
            class="form-select"
            onchange="this.closest('ui-settings').handleThemeChange(this.value)"
          >
            <option value="system" ${currentTheme === 'system' ? 'selected' : ''}>Auto</option>
            <option value="dark" ${currentTheme === 'dark' ? 'selected' : ''}>Dark</option>
            <option value="light" ${currentTheme === 'light' ? 'selected' : ''}>Light</option>
          </select>
        </div>

        <div class="form-group">
          <label class="checkbox-label">
            <input
              type="checkbox"
              ${ui?.enableColorImpairedMode ? 'checked' : ''}
            />
            <span>Enable Color-Impaired Mode</span>
          </label>
          <p class="form-hint">Enables additional text and visual cues for colorblind users</p>
        </div>
      </div>

      <div class="settings-section">
        <h2 class="section-title">Language</h2>

        <div class="form-group">
          <label class="form-label">UI Language</label>
          <select class="form-select">
            <option value="1">English</option>
            <option value="2">French</option>
            <option value="3">Spanish</option>
            <option value="4">German</option>
            <option value="5">Italian</option>
            <option value="6">Danish</option>
            <option value="7">Dutch</option>
            <option value="8">Japanese</option>
            <option value="9">Icelandic</option>
            <option value="10">Chinese</option>
            <option value="11">Russian</option>
            <option value="12">Polish</option>
            <option value="13">Vietnamese</option>
            <option value="14">Swedish</option>
            <option value="15">Norwegian</option>
            <option value="16">Finnish</option>
            <option value="17">Turkish</option>
            <option value="18">Portuguese</option>
            <option value="19">Flemish</option>
            <option value="20">Greek</option>
            <option value="21">Korean</option>
            <option value="22">Hungarian</option>
            <option value="23">Hebrew</option>
            <option value="24">Lithuanian</option>
            <option value="25">Czech</option>
            <option value="26">Arabic</option>
            <option value="27">Hindi</option>
            <option value="28">Bulgarian</option>
            <option value="29">Malayalam</option>
            <option value="30">Ukrainian</option>
            <option value="31">Slovak</option>
            <option value="32">Thai</option>
            <option value="33">Portuguese (Brazil)</option>
            <option value="34">Catalan</option>
            <option value="35">Spanish (Latino)</option>
            <option value="36">Romanian</option>
            <option value="37">Croatian</option>
            <option value="38">Indonesian</option>
            <option value="39">Serbian</option>
            <option value="40">Slovenian</option>
            <option value="41">Estonian</option>
            <option value="42">Latvian</option>
            <option value="43">Macedonian</option>
          </select>
        </div>
      </div>

      <div class="actions">
        <button class="save-btn" onclick="this.closest('ui-settings').handleSave()">
          Save Changes
        </button>
      </div>

      <style>
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

        .settings-section {
          margin-bottom: 2rem;
          padding: 1.5rem;
          background-color: var(--bg-card);
          border: 1px solid var(--border-color);
          border-radius: 0.5rem;
        }

        .section-title {
          font-size: 1.125rem;
          font-weight: 600;
          margin: 0 0 1.5rem 0;
          padding-bottom: 0.75rem;
          border-bottom: 1px solid var(--border-color);
        }

        .form-group {
          margin-bottom: 1.25rem;
        }

        .form-group:last-child {
          margin-bottom: 0;
        }

        .form-label {
          display: block;
          font-size: 0.875rem;
          font-weight: 500;
          margin-bottom: 0.5rem;
          color: var(--text-color);
        }

        .form-select {
          width: 100%;
          max-width: 400px;
          padding: 0.5rem 0.75rem;
          font-size: 0.875rem;
          background-color: var(--bg-input);
          border: 1px solid var(--border-input);
          border-radius: 0.25rem;
          color: var(--text-color);
        }

        .form-select:focus {
          outline: none;
          border-color: var(--color-primary);
        }

        .form-hint {
          font-size: 0.75rem;
          color: var(--text-color-muted);
          margin: 0.25rem 0 0 0;
        }

        .checkbox-label {
          display: flex;
          align-items: center;
          gap: 0.5rem;
          cursor: pointer;
        }

        .checkbox-label input[type="checkbox"] {
          width: 16px;
          height: 16px;
          accent-color: var(--color-primary);
        }

        .actions {
          margin-top: 1.5rem;
        }

        .save-btn {
          padding: 0.625rem 1.25rem;
          background-color: var(--btn-primary-bg);
          border: 1px solid var(--btn-primary-border);
          border-radius: 0.25rem;
          color: var(--color-white);
          font-size: 0.875rem;
          font-weight: 500;
          cursor: pointer;
        }

        .save-btn:hover {
          background-color: var(--btn-primary-bg-hover);
        }
      </style>
    `;
  }

  handleThemeChange(value: string): void {
    setTheme(value as Theme);
  }

  handleSave(): void {
    showSuccess('Settings saved');
  }
}
