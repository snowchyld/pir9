/**
 * TanStack Query Core wrapper for data fetching and caching
 * Provides reactive query state that integrates with signals
 */

import {
  MutationObserver,
  type MutationObserverOptions,
  type MutationObserverResult,
  QueryClient,
  type QueryKey,
  QueryObserver,
  type QueryObserverOptions,
  type QueryObserverResult,
} from '@tanstack/query-core';
import { http } from './http';
import { batch, type Signal, signal } from './reactive';

/**
 * Singleton query client instance
 */
export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 5 * 60 * 1000, // 5 minutes
      gcTime: 30 * 60 * 1000, // 30 minutes (was cacheTime)
      retry: 1,
      refetchOnWindowFocus: false, // Disabled — causes render storms on mobile
    },
    mutations: {
      retry: 0,
    },
  },
});

/**
 * Query state that can be watched by components
 */
export interface QueryState<TData> {
  data: Signal<TData | undefined>;
  error: Signal<Error | null>;
  isLoading: Signal<boolean>;
  isFetching: Signal<boolean>;
  isError: Signal<boolean>;
  isSuccess: Signal<boolean>;
  refetch: () => Promise<QueryObserverResult<TData, Error>>;
}

/**
 * Mutation state that can be watched by components
 */
export interface MutationState<TData, TVariables> {
  data: Signal<TData | undefined>;
  error: Signal<Error | null>;
  isLoading: Signal<boolean>;
  isError: Signal<boolean>;
  isSuccess: Signal<boolean>;
  mutate: (variables: TVariables) => void;
  mutateAsync: (variables: TVariables) => Promise<TData>;
  reset: () => void;
}

/**
 * Create a reactive query
 */
export function createQuery<TData>(
  options: Omit<QueryObserverOptions<TData, Error, TData, TData, QueryKey>, 'queryClient'>,
): QueryState<TData> {
  const data = signal<TData | undefined>(undefined);
  const error = signal<Error | null>(null);
  const isLoading = signal(true);
  const isFetching = signal(true);
  const isError = signal(false);
  const isSuccess = signal(false);

  const observer = new QueryObserver<TData, Error, TData, TData, QueryKey>(queryClient, options);

  // Update signals when query state changes (batched to avoid cascading renders)
  observer.subscribe((result: QueryObserverResult<TData, Error>) => {
    batch(() => {
      data.set(result.data);
      error.set(result.error);
      isLoading.set(result.isLoading);
      isFetching.set(result.isFetching);
      isError.set(result.isError);
      isSuccess.set(result.isSuccess);
    });
  });

  // Get initial state (batched)
  const initialResult = observer.getCurrentResult();
  batch(() => {
    data.set(initialResult.data);
    error.set(initialResult.error);
    isLoading.set(initialResult.isLoading);
    isFetching.set(initialResult.isFetching);
    isError.set(initialResult.isError);
    isSuccess.set(initialResult.isSuccess);
  });

  return {
    data,
    error,
    isLoading,
    isFetching,
    isError,
    isSuccess,
    refetch: () => observer.refetch(),
  };
}

/**
 * Create a reactive mutation
 */
export function createMutation<TData, TVariables>(
  options: Omit<MutationObserverOptions<TData, Error, TVariables, unknown>, 'mutationFn'> & {
    mutationFn: (variables: TVariables) => Promise<TData>;
  },
): MutationState<TData, TVariables> {
  const data = signal<TData | undefined>(undefined);
  const error = signal<Error | null>(null);
  const isLoading = signal(false);
  const isError = signal(false);
  const isSuccess = signal(false);

  const observer = new MutationObserver<TData, Error, TVariables, unknown>(queryClient, options);

  // Update signals when mutation state changes (batched)
  observer.subscribe((result: MutationObserverResult<TData, Error, TVariables, unknown>) => {
    batch(() => {
      data.set(result.data);
      error.set(result.error);
      isLoading.set(result.isPending);
      isError.set(result.isError);
      isSuccess.set(result.isSuccess);
    });
  });

  return {
    data,
    error,
    isLoading,
    isError,
    isSuccess,
    mutate: (variables: TVariables) => {
      observer.mutate(variables);
    },
    mutateAsync: (variables: TVariables) => observer.mutate(variables),
    reset: () => observer.reset(),
  };
}

/**
 * Invalidate queries by key
 */
export function invalidateQueries(queryKey: QueryKey): Promise<void> {
  return queryClient.invalidateQueries({ queryKey });
}

/**
 * Set query data directly (for optimistic updates)
 */
export function setQueryData<TData>(
  queryKey: QueryKey,
  updater: TData | ((old: TData | undefined) => TData | undefined),
): void {
  queryClient.setQueryData(queryKey, updater);
}

/**
 * Get cached query data
 */
export function getQueryData<TData>(queryKey: QueryKey): TData | undefined {
  return queryClient.getQueryData(queryKey);
}

/**
 * Prefetch query data
 */
export function prefetchQuery<TData>(
  options: Omit<QueryObserverOptions<TData, Error, TData, TData, QueryKey>, 'queryClient'>,
): Promise<void> {
  return queryClient.prefetchQuery(options);
}

// Convenience query factories for common endpoints

/**
 * Create a query for fetching series list
 */
export function useSeriesQuery() {
  return createQuery({
    queryKey: ['/series'],
    queryFn: () => http.get<import('./http').Series[]>('/series'),
  });
}

/**
 * Create a query for fetching a single series
 */
export function useSeriesDetailQuery(id: number) {
  return createQuery({
    queryKey: ['/series', id],
    queryFn: () => http.get<import('./http').Series>(`/series/${id}`),
    enabled: id > 0,
  });
}

/**
 * Create a query for fetching movies list
 */
export function useMoviesQuery() {
  return createQuery({
    queryKey: ['/movie'],
    queryFn: () => http.get<import('./http').Movie[]>('/movie'),
  });
}

/**
 * Create a query for fetching a single movie
 */
export function useMovieDetailQuery(id: number) {
  return createQuery({
    queryKey: ['/movie', id],
    queryFn: () => http.get<import('./http').Movie>(`/movie/${id}`),
    enabled: id > 0,
  });
}

/**
 * Create a query for fetching artists list
 */
export function useArtistsQuery() {
  return createQuery({
    queryKey: ['/artist'],
    queryFn: () => http.get<import('./http').Artist[]>('/artist'),
  });
}

/**
 * Create a query for fetching podcasts list
 */
export function usePodcastsQuery() {
  return createQuery({
    queryKey: ['/podcast'],
    queryFn: () => http.get<import('./http').Podcast[]>('/podcast'),
  });
}

/**
 * Create a query for fetching audiobooks list
 */
export function useAudiobooksQuery() {
  return createQuery({
    queryKey: ['/audiobook'],
    queryFn: () => http.get<import('./http').Audiobook[]>('/audiobook'),
  });
}

/**
 * Create a query for fetching episodes
 */
export function useEpisodesQuery(seriesId: number) {
  return createQuery({
    queryKey: ['/episode', { seriesId }],
    queryFn: () => http.get<import('./http').Episode[]>('/episode', { params: { seriesId } }),
    enabled: seriesId > 0,
  });
}

/**
 * Create a query for calendar events
 */
export function useCalendarQuery(start: string, end: string) {
  return createQuery({
    queryKey: ['/calendar', { start, end }],
    queryFn: () =>
      http.get<import('./http').CalendarEvent[]>('/calendar', { params: { start, end } }),
  });
}

/**
 * Create a query for queue
 */
export function useQueueQuery() {
  return createQuery({
    queryKey: ['/queue'],
    queryFn: () =>
      http.get<import('./http').QueueResponse>('/queue', {
        params: { pageSize: 10000 },
      }),
    refetchInterval: 5000, // Poll every 5 seconds
  });
}

/**
 * Create a query for a specific content type queue
 */
export function useContentQueueQuery(contentType: string) {
  return createQuery({
    queryKey: ['/queue', contentType],
    queryFn: () =>
      http.get<import('./http').QueueResponse>(`/queue/${contentType}`, {
        params: { pageSize: 10000 },
      }),
    refetchInterval: 5000,
  });
}

/**
 * Create a query for system status
 */
export function useSystemStatusQuery() {
  return createQuery({
    queryKey: ['/system/status'],
    queryFn: () => http.get<import('./http').SystemStatus>('/system/status'),
    staleTime: 60 * 1000, // 1 minute
  });
}

/**
 * Create a query for health checks
 */
export function useHealthQuery() {
  return createQuery({
    queryKey: ['/health'],
    queryFn: () => http.get<import('./http').HealthCheck[]>('/health'),
    refetchInterval: 30000,
  });
}

/**
 * Create a query for disk space
 */
export function useDiskSpaceQuery() {
  return createQuery({
    queryKey: ['/system/diskspace'],
    queryFn: () => http.get<import('./http').DiskSpace[]>('/system/diskspace'),
    staleTime: 60 * 1000,
  });
}

/**
 * Create a query for system updates
 */
export function useUpdateQuery() {
  return createQuery({
    queryKey: ['/system/update'],
    queryFn: () => http.get<import('./http').UpdateInfo>('/system/update'),
    staleTime: 5 * 60 * 1000,
  });
}

/**
 * Create a mutation for executing commands
 */
export function useCommandMutation() {
  return createMutation({
    mutationFn: (command: { name: string; [key: string]: unknown }) =>
      http.post<import('./http').Command>('/command', command),
    onSuccess: () => {
      invalidateQueries(['/command']);
    },
  });
}
