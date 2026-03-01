/**
 * WebSocket manager for real-time updates
 * Handles connection, reconnection, and message routing
 */

import type { Command } from './http';
import { getQueryData, invalidateQueries, setQueryData } from './query';
import { type Signal, signal } from './reactive';

export type ConnectionState = 'connecting' | 'connected' | 'disconnected' | 'error';

export interface WebSocketMessage {
  type: string;
  [key: string]: unknown;
}

export interface CommandMessage extends WebSocketMessage {
  type: 'command_started' | 'command_updated' | 'command_completed' | 'command_failed';
  command_id: number;
  name: string;
  message?: string;
  error?: string;
}

export interface SeriesMessage extends WebSocketMessage {
  type:
    | 'series_refreshed'
    | 'series_scanned'
    | 'series_updated'
    | 'series_added'
    | 'series_deleted';
  series_id: number;
  title: string;
  files_found?: number;
  episodes_matched?: number;
}

export interface EpisodeMessage extends WebSocketMessage {
  type:
    | 'episode_grabbed'
    | 'episode_imported'
    | 'episode_file_imported'
    | 'episode_renamed'
    | 'episode_deleted';
  series_id: number;
  episode_id?: number;
  episode_ids?: number[];
  title?: string;
}

export interface QueueMessage extends WebSocketMessage {
  type: 'queue_updated' | 'queue_item_removed';
  queue_id?: number;
}

type MessageHandler = (message: WebSocketMessage) => void;

/**
 * WebSocket manager singleton
 */
class WebSocketManager {
  private ws: WebSocket | null = null;
  private reconnectTimer: number | null = null;
  private reconnectAttempts = 0;
  private maxReconnectAttempts = 10;
  private reconnectDelay = 1000; // Start with 1 second
  private handlers = new Map<string, Set<MessageHandler>>();

  readonly connectionState: Signal<ConnectionState> = signal('disconnected');
  readonly lastMessage: Signal<WebSocketMessage | null> = signal(null);

  /**
   * Connect to WebSocket server
   */
  connect(): void {
    if (this.ws?.readyState === WebSocket.OPEN) {
      return;
    }

    this.connectionState.set('connecting');

    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const wsUrl = `${protocol}//${window.location.host}/ws`;

    console.log('[WebSocket] Connecting to:', wsUrl);

    try {
      this.ws = new WebSocket(wsUrl);
      this.setupEventHandlers();
    } catch (error) {
      console.error('[WebSocket] Connection error:', error);
      this.connectionState.set('error');
      this.scheduleReconnect();
    }
  }

  /**
   * Disconnect from WebSocket server
   */
  disconnect(): void {
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }

    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }

    this.connectionState.set('disconnected');
    this.reconnectAttempts = 0;
  }

  /**
   * Subscribe to messages of a specific type
   */
  on(type: string, handler: MessageHandler): () => void {
    if (!this.handlers.has(type)) {
      this.handlers.set(type, new Set());
    }
    this.handlers.get(type)?.add(handler);

    // Return unsubscribe function
    return () => {
      this.handlers.get(type)?.delete(handler);
    };
  }

  /**
   * Subscribe to all messages
   */
  onAny(handler: MessageHandler): () => void {
    return this.on('*', handler);
  }

  /**
   * Send a message to the server
   */
  send(message: WebSocketMessage): void {
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(message));
    } else {
      console.warn('[WebSocket] Cannot send message, not connected');
    }
  }

  private setupEventHandlers(): void {
    if (!this.ws) return;

    this.ws.onopen = () => {
      console.log('[WebSocket] Connected');
      this.connectionState.set('connected');
      this.reconnectAttempts = 0;
      this.reconnectDelay = 1000;
    };

    this.ws.onclose = (event) => {
      console.log('[WebSocket] Disconnected:', event.code, event.reason);
      this.ws = null;
      this.connectionState.set('disconnected');

      // Don't reconnect if closed cleanly (code 1000) or intentionally
      if (event.code !== 1000) {
        this.scheduleReconnect();
      }
    };

    this.ws.onerror = (error) => {
      console.error('[WebSocket] Error:', error);
      this.connectionState.set('error');
    };

    this.ws.onmessage = (event) => {
      try {
        const message = JSON.parse(event.data) as WebSocketMessage;
        this.handleMessage(message);
      } catch (error) {
        console.error('[WebSocket] Failed to parse message:', error);
      }
    };
  }

  private handleMessage(message: WebSocketMessage): void {
    console.log('[WebSocket] Received:', message.type, message);

    this.lastMessage.set(message);

    // Notify type-specific handlers
    this.handlers.get(message.type)?.forEach((handler) => {
      handler(message);
    });

    // Notify wildcard handlers
    this.handlers.get('*')?.forEach((handler) => {
      handler(message);
    });

    // Handle built-in message types
    this.handleBuiltInMessage(message);
  }

  private handleBuiltInMessage(message: WebSocketMessage): void {
    switch (message.type) {
      case 'command_started':
      case 'command_updated':
      case 'command_completed':
      case 'command_failed':
        this.handleCommandMessage(message as CommandMessage);
        break;

      case 'series_refreshed':
      case 'series_scanned':
      case 'series_updated':
      case 'series_added':
      case 'series_deleted':
        this.handleSeriesMessage(message as SeriesMessage);
        break;

      case 'episode_grabbed':
      case 'episode_imported':
      case 'episode_file_imported':
      case 'episode_renamed':
      case 'episode_deleted':
        this.handleEpisodeMessage(message as EpisodeMessage);
        break;

      case 'movie_updated':
      case 'movie_refreshed':
      case 'movie_added':
      case 'movie_deleted':
      case 'movie_file_imported':
      case 'movie_file_deleted':
        this.handleMovieMessage(message);
        break;

      case 'queue_updated':
      case 'queue_item_removed':
        this.handleQueueMessage(message as QueueMessage);
        break;
    }
  }

  private handleCommandMessage(message: CommandMessage): void {
    const commands = getQueryData<Command[]>(['/command']);

    if (!commands) {
      invalidateQueries(['/command']);
      return;
    }

    const commandIndex = commands.findIndex((c) => c.id === message.command_id);

    if (commandIndex === -1) {
      invalidateQueries(['/command']);
      return;
    }

    // Map message type to command status
    let status: Command['status'] = 'queued';

    switch (message.type) {
      case 'command_started':
        status = 'started';
        break;
      case 'command_completed':
        status = 'completed';
        break;
      case 'command_failed':
        status = 'failed';
        break;
    }

    const updatedCommand: Command = {
      ...commands[commandIndex],
      status,
      message: message.message ?? commands[commandIndex].message,
    };

    const newCommands = [...commands];
    newCommands[commandIndex] = updatedCommand;

    setQueryData(['/command'], newCommands);
  }

  private handleSeriesMessage(message: SeriesMessage): void {
    console.log(
      '[WebSocket] Series event:',
      message.type,
      message.title,
      'series_id:',
      message.series_id,
    );

    // Invalidate ALL series and episode queries
    // TanStack Query uses prefix matching, so ['/series'] will match all series queries
    console.log('[WebSocket] Invalidating series and episode queries...');

    // Invalidate all series queries (matches ['/series'], ['/series', id], ['/series', null], etc.)
    invalidateQueries(['/series']);

    // Invalidate all episode queries (matches ['/episode'], ['/episode', id], ['/episode', {seriesId}], etc.)
    invalidateQueries(['/episode']);

    console.log('[WebSocket] Query invalidation complete');
  }

  private handleEpisodeMessage(message: EpisodeMessage): void {
    console.log('[WebSocket] Episode event:', message.type);

    // Invalidate episode queries
    invalidateQueries(['/episode', { seriesId: message.series_id }]);

    // Invalidate queue and history
    invalidateQueries(['/queue']);
    invalidateQueries(['/history']);
  }

  private handleMovieMessage(message: WebSocketMessage): void {
    console.log('[WebSocket] Movie event:', message.type);

    invalidateQueries(['/movie']);
  }

  private handleQueueMessage(_message: QueueMessage): void {
    // Invalidate queue queries
    invalidateQueries(['/queue']);
  }

  private scheduleReconnect(): void {
    if (this.reconnectAttempts >= this.maxReconnectAttempts) {
      console.log('[WebSocket] Max reconnect attempts reached');
      return;
    }

    this.reconnectAttempts++;

    // Exponential backoff with jitter
    const delay = Math.min(
      this.reconnectDelay * 2 ** (this.reconnectAttempts - 1) + Math.random() * 1000,
      30000, // Max 30 seconds
    );

    console.log(
      `[WebSocket] Reconnecting in ${Math.round(delay / 1000)}s (attempt ${this.reconnectAttempts})`,
    );

    this.reconnectTimer = window.setTimeout(() => {
      this.connect();
    }, delay);
  }
}

/**
 * Singleton WebSocket manager instance
 */
export const wsManager = new WebSocketManager();

/**
 * Initialize WebSocket connection
 * Call this once when the app starts
 */
export function initializeWebSocket(): void {
  wsManager.connect();
}
