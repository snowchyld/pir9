/**
 * HTTP API client for /api/v5 endpoints
 * Provides typed fetch wrapper with error handling
 */

const API_BASE = '/api/v5';
const API_BASE_V3 = '/api/v3';

export interface ApiError {
  status: number;
  message: string;
  errors?: Record<string, string[]>;
}

export class HttpError extends Error {
  constructor(
    public status: number,
    message: string,
    public errors?: Record<string, string[]>
  ) {
    super(message);
    this.name = 'HttpError';
  }
}

interface RequestOptions extends Omit<RequestInit, 'body'> {
  params?: Record<string, string | number | boolean | undefined>;
  body?: unknown;
  apiBase?: string;
}

/**
 * Build URL with query parameters
 */
function buildUrl(
  path: string,
  params?: Record<string, string | number | boolean | undefined>,
  apiBase: string = API_BASE
): string {
  const url = new URL(`${apiBase}${path}`, window.location.origin);

  if (params) {
    Object.entries(params).forEach(([key, value]) => {
      if (value !== undefined) {
        url.searchParams.set(key, String(value));
      }
    });
  }

  return url.toString();
}

/**
 * Parse response based on content type
 */
async function parseResponse<T>(response: Response): Promise<T> {
  const contentType = response.headers.get('content-type');

  if (contentType?.includes('application/json')) {
    return response.json() as Promise<T>;
  }

  // Return text for non-JSON responses
  return response.text() as unknown as Promise<T>;
}

/**
 * Handle API errors
 */
async function handleError(response: Response): Promise<never> {
  let message = `HTTP ${response.status}: ${response.statusText}`;
  let errors: Record<string, string[]> | undefined;

  try {
    const body = await response.json();
    if (body.message) {
      message = body.message;
    }
    if (body.errors) {
      errors = body.errors;
    }
  } catch {
    // Response wasn't JSON, use default message
  }

  throw new HttpError(response.status, message, errors);
}

/**
 * Core fetch wrapper
 */
async function request<T>(
  method: string,
  path: string,
  options: RequestOptions = {}
): Promise<T> {
  const { params, body, headers: customHeaders, apiBase, ...fetchOptions } = options;

  const url = buildUrl(path, params, apiBase);

  const headers: HeadersInit = {
    'Content-Type': 'application/json',
    ...customHeaders,
  };

  const config: RequestInit = {
    method,
    headers,
    ...fetchOptions,
  };

  if (body !== undefined) {
    config.body = JSON.stringify(body);
  }

  const response = await fetch(url, config);

  if (!response.ok) {
    return handleError(response);
  }

  // Handle 204 No Content
  if (response.status === 204) {
    return undefined as T;
  }

  return parseResponse<T>(response);
}

/**
 * HTTP client with typed methods
 */
export const http = {
  get<T>(path: string, options?: RequestOptions): Promise<T> {
    return request<T>('GET', path, options);
  },

  post<T>(path: string, body?: unknown, options?: RequestOptions): Promise<T> {
    return request<T>('POST', path, { ...options, body });
  },

  put<T>(path: string, body?: unknown, options?: RequestOptions): Promise<T> {
    return request<T>('PUT', path, { ...options, body });
  },

  patch<T>(path: string, body?: unknown, options?: RequestOptions): Promise<T> {
    return request<T>('PATCH', path, { ...options, body });
  },

  delete<T>(path: string, options?: RequestOptions): Promise<T> {
    return request<T>('DELETE', path, options);
  },
};

/**
 * HTTP client for v3 API endpoints (providers use v3)
 */
export const httpV3 = {
  get<T>(path: string, options?: RequestOptions): Promise<T> {
    return request<T>('GET', path, { ...options, apiBase: API_BASE_V3 });
  },

  post<T>(path: string, body?: unknown, options?: RequestOptions): Promise<T> {
    return request<T>('POST', path, { ...options, body, apiBase: API_BASE_V3 });
  },

  put<T>(path: string, body?: unknown, options?: RequestOptions): Promise<T> {
    return request<T>('PUT', path, { ...options, body, apiBase: API_BASE_V3 });
  },

  delete<T>(path: string, options?: RequestOptions): Promise<T> {
    return request<T>('DELETE', path, { ...options, apiBase: API_BASE_V3 });
  },
};

/**
 * Type-safe API client for common endpoints
 */
export const api = {
  // Series
  series: {
    list: () => http.get<Series[]>('/series'),
    get: (id: number) => http.get<Series>(`/series/${id}`),
    create: (data: Partial<Series>) => http.post<Series>('/series', data),
    update: (id: number, data: Partial<Series>) =>
      http.put<Series>(`/series/${id}`, data),
    delete: (id: number, params?: { deleteFiles?: boolean }) =>
      http.delete<void>(`/series/${id}`, { params }),
  },

  // Episodes
  episode: {
    list: (seriesId: number) =>
      http.get<Episode[]>('/episode', { params: { seriesId } }),
    get: (id: number) => http.get<Episode>(`/episode/${id}`),
  },

  // Calendar
  calendar: {
    get: (start: string, end: string) =>
      http.get<CalendarEvent[]>('/calendar', { params: { start, end } }),
  },

  // Queue
  queue: {
    list: () => http.get<QueueResponse>('/queue'),
    delete: (id: number, params?: { removeFromClient?: boolean; blocklist?: boolean }) =>
      http.delete<void>(`/queue/${id}`, { params }),
  },

  // History
  history: {
    list: (params?: { page?: number; pageSize?: number; seriesId?: number }) =>
      http.get<HistoryResponse>('/history', { params }),
  },

  // Commands
  command: {
    list: () => http.get<Command[]>('/command'),
    execute: (name: string, body?: Record<string, unknown>) =>
      http.post<Command>('/command', { name, ...body }),
  },

  // Movies
  movie: {
    list: () => http.get<Movie[]>('/movie'),
    get: (id: number) => http.get<Movie>(`/movie/${id}`),
    create: (data: Partial<Movie>) => http.post<Movie>('/movie', data),
    update: (id: number, data: Partial<Movie>) =>
      http.put<Movie>(`/movie/${id}`, data),
    delete: (id: number, params?: { deleteFiles?: boolean }) =>
      http.delete<void>(`/movie/${id}`, { params }),
    lookup: (term: string) =>
      http.get<MovieLookupResult[]>('/movie/lookup', { params: { term } }),
  },

  // System
  system: {
    status: () => http.get<SystemStatus>('/system/status'),
  },
};

// Type definitions (matching existing frontend types)
export interface Series {
  id: number;
  title: string;
  titleSlug: string;
  sortTitle: string;
  status: 'continuing' | 'ended' | 'upcoming' | 'deleted';
  overview: string;
  network: string;
  year: number;
  path: string;
  qualityProfileId: number;
  seasonFolder: boolean;
  monitored: boolean;
  seriesType: 'anime' | 'daily' | 'standard';
  runtime: number;
  tvdbId: number;
  tvRageId: number;
  tvMazeId: number;
  tmdbId: number;
  imdbId?: string;
  certification?: string;
  genres: string[];
  tags: number[];
  added: string;
  firstAired?: string;
  previousAiring?: string;
  nextAiring?: string;
  images: SeriesImage[];
  seasons: Season[];
  statistics?: SeriesStatistics;
}

export interface SeriesImage {
  coverType: 'poster' | 'banner' | 'fanart';
  url: string;
  remoteUrl: string;
}

export interface Season {
  seasonNumber: number;
  monitored: boolean;
  statistics?: SeasonStatistics;
}

export interface SeriesStatistics {
  seasonCount: number;
  episodeCount: number;
  episodeFileCount: number;
  totalEpisodeCount: number;
  sizeOnDisk: number;
  percentOfEpisodes: number;
}

export interface SeasonStatistics {
  episodeCount: number;
  episodeFileCount: number;
  totalEpisodeCount: number;
  sizeOnDisk: number;
  percentOfEpisodes: number;
}

export interface Episode {
  id: number;
  seriesId: number;
  episodeFileId: number;
  seasonNumber: number;
  episodeNumber: number;
  title: string;
  airDate?: string;
  airDateUtc?: string;
  overview?: string;
  hasFile: boolean;
  monitored: boolean;
}

export interface CalendarEvent extends Episode {
  series: Series;
}

export interface Movie {
  id: number;
  title: string;
  sortTitle: string;
  titleSlug: string;
  status: 'tba' | 'announced' | 'inCinemas' | 'released' | 'deleted';
  overview?: string;
  year: number;
  studio?: string;
  path: string;
  rootFolderPath: string;
  qualityProfileId: number;
  monitored: boolean;
  runtime: number;
  tmdbId: number;
  imdbId?: string;
  certification?: string;
  genres: string[];
  tags: number[];
  added: string;
  releaseDate?: string;
  physicalReleaseDate?: string;
  digitalReleaseDate?: string;
  images: SeriesImage[];
  hasFile: boolean;
  movieFileId?: number;
  cleanTitle: string;
  folder?: string;
  ratings?: { votes: number; value: number };
  imdbRating?: number;
  imdbVotes?: number;
  statistics?: MovieStatistics;
}

export interface MovieStatistics {
  sizeOnDisk: number;
  hasFile: boolean;
}

export interface MovieLookupResult {
  imdbId?: string;
  title: string;
  sortTitle: string;
  overview?: string;
  year: number;
  studio?: string;
  images: SeriesImage[];
  ratings?: { votes: number; value: number };
  genres: string[];
  runtime: number;
  certification?: string;
}

export interface QueueItemEpisode {
  id: number;
  seasonNumber: number;
  episodeNumber: number;
  title: string;
  airDateUtc?: string;
}

export interface QueueItemSeries {
  id: number;
  title: string;
}

export interface QueueItem {
  id: number;
  seriesId?: number | null;
  episodeId?: number | null;
  title: string;
  status: string;
  trackedDownloadStatus: string;
  statusMessages: { title: string; messages: string[] }[];
  downloadId: string;
  protocol: 'usenet' | 'torrent';
  downloadClient: string;
  size: number;
  sizeleft: number;
  timeleft?: string;
  estimatedCompletionTime?: string;
  added?: string;
  episode?: QueueItemEpisode;
  series?: QueueItemSeries;
}

export interface QueueResponse {
  page: number;
  pageSize: number;
  totalRecords: number;
  records: QueueItem[];
}

export interface HistoryRecord {
  id: number;
  seriesId: number;
  episodeId: number;
  eventType: string;
  sourceTitle: string;
  date: string;
  quality: { quality: { name: string } };
}

export interface HistoryResponse {
  page: number;
  pageSize: number;
  totalRecords: number;
  records: HistoryRecord[];
}

export interface Command {
  id: number;
  name: string;
  status: 'queued' | 'started' | 'completed' | 'failed';
  message?: string;
  started?: string;
  ended?: string;
}

export interface SystemStatus {
  version: string;
  buildTime: string;
  isDebug: boolean;
  isProduction: boolean;
  isAdmin: boolean;
  isUserInteractive: boolean;
  startupPath: string;
  appData: string;
  osName: string;
  osVersion: string;
  isDocker: boolean;
  isLinux: boolean;
  isOsx: boolean;
  isWindows: boolean;
  branch: string;
  authentication: string;
  urlBase: string;
}
