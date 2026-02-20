/**
 * Type definitions for provider-based settings (download clients, indexers, etc.)
 */

export interface ProviderField {
  order: number;
  name: string;
  label: string;
  unit?: string;
  helpText?: string;
  helpTextWarning?: string;
  helpLink?: string;
  value: unknown;
  type: string; // textbox, password, checkbox, select, number, path, url, tag
  advanced: boolean;
  selectOptions?: SelectOption[];
  selectOptionsProviderAction?: string;
  section?: string;
  hidden?: 'hidden' | 'hiddenIfNotSet' | 'visible';
  privacy?: string;
  placeholder?: string;
  isFloat?: boolean;
}

export interface SelectOption {
  value: number | string;
  name: string;
  order: number;
  hint?: string;
}

export interface ProviderMessage {
  message: string;
  type: 'info' | 'warning' | 'error';
}

export interface ProviderSchema {
  id: number;
  name: string;
  fields: ProviderField[];
  implementationName: string;
  implementation: string;
  configContract: string;
  infoLink?: string;
  message?: ProviderMessage;
  tags: number[];
  presets?: ProviderSchema[];
  enable: boolean;
  protocol?: string; // for download clients
  priority?: number;
}

export interface DownloadClientSchema extends ProviderSchema {
  protocol: 'usenet' | 'torrent';
  priority: number;
  removeCompletedDownloads: boolean;
  removeFailedDownloads: boolean;
}

export interface IndexerSchema extends ProviderSchema {
  enableRss: boolean;
  enableAutomaticSearch: boolean;
  enableInteractiveSearch: boolean;
  supportsRss: boolean;
  supportsSearch: boolean;
  protocol: 'usenet' | 'torrent';
  priority: number;
  downloadClientId: number;
}

export interface NotificationSchema extends ProviderSchema {
  onGrab: boolean;
  onDownload: boolean;
  onUpgrade: boolean;
  onRename: boolean;
  onSeriesAdd: boolean;
  onSeriesDelete: boolean;
  onEpisodeFileDelete: boolean;
  onEpisodeFileDeleteForUpgrade: boolean;
  onHealthIssue: boolean;
  onHealthRestored: boolean;
  onApplicationUpdate: boolean;
  onManualInteractionRequired: boolean;
  supportsOnGrab: boolean;
  supportsOnDownload: boolean;
  supportsOnUpgrade: boolean;
  supportsOnRename: boolean;
  supportsOnSeriesAdd: boolean;
  supportsOnSeriesDelete: boolean;
  supportsOnEpisodeFileDelete: boolean;
  supportsOnEpisodeFileDeleteForUpgrade: boolean;
  supportsOnHealthIssue: boolean;
  supportsOnHealthRestored: boolean;
  supportsOnApplicationUpdate: boolean;
  supportsOnManualInteractionRequired: boolean;
}

export interface ImportListSchema extends ProviderSchema {
  enableAutomaticAdd: boolean;
  shouldMonitor: string;
  rootFolderPath: string;
  qualityProfileId: number;
  seriesType: string;
  seasonFolder: boolean;
  listType: string;
  listOrder: number;
}

export type AnyProviderSchema = DownloadClientSchema | IndexerSchema | NotificationSchema | ImportListSchema | ProviderSchema;
