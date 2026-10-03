import type { TerminalChunkDto, TerminalSnapshotDto } from '@/lib/tauri/commands';

/** Output kept per execution in the webview; the core keeps its own (bounded) copy too. */
export const TERMINAL_OUTPUT_LIMIT = 512 * 1024;
/** Executions whose output is kept, newest last; older ones are forgotten. */
const KEPT_EXECUTIONS = 8;

interface Chunk {
  seq: number;
  data: string;
}

interface Buffer {
  chunks: Chunk[];
  size: number;
  /** The `seq` the next chunk must have: anything below it is already in `chunks`. */
  nextSeq: number;
}

type Listener = (data: string) => void;

/**
 * The terminal output of every execution, outside React state: output can arrive many times a
 * second and must not re-render the app per chunk. A terminal view reads what is there, then
 * subscribes to what follows.
 *
 * It reconstructs output from the core's incremental events (`execution:output`), and from a
 * snapshot when a view opens late (`load`); `seq` makes the two meet without a gap or a repeat.
 * The output is ephemeral: it is never saved.
 */
export class TerminalHub {
  private readonly buffers = new Map<string, Buffer>();
  private readonly listeners = new Map<string, Set<Listener>>();

  /** Adds a chunk, unless it is one the buffer already has. */
  push(chunk: TerminalChunkDto): void {
    const buffer = this.buffer(chunk.executionId);
    if (chunk.seq < buffer.nextSeq) return;
    buffer.nextSeq = chunk.seq + 1;
    append(buffer, { seq: chunk.seq, data: chunk.data });
    this.listeners.get(chunk.executionId)?.forEach((listener) => {
      listener(chunk.data);
    });
  }

  /**
   * Takes the core's snapshot as the base and keeps the chunks received after it: the snapshot is
   * the truth up to its `nextSeq`, whatever this hub missed before. Returns the text a view
   * should show now. Listeners are not told: the caller redraws from the result.
   */
  load(snapshot: TerminalSnapshotDto): string {
    const buffer = this.buffer(snapshot.executionId);
    const after = buffer.chunks.filter((chunk) => chunk.seq >= snapshot.nextSeq);
    buffer.chunks = [];
    buffer.size = 0;
    append(buffer, { seq: snapshot.nextSeq - 1, data: snapshot.output });
    after.forEach((chunk) => {
      append(buffer, chunk);
    });
    buffer.nextSeq = Math.max(buffer.nextSeq, snapshot.nextSeq);
    return text(buffer);
  }

  read(executionId: string): string {
    const buffer = this.buffers.get(executionId);
    return buffer ? text(buffer) : '';
  }

  /** Forgets the text (the user cleared the terminal); later chunks still arrive. */
  clear(executionId: string): void {
    const buffer = this.buffers.get(executionId);
    if (buffer) {
      buffer.chunks = [];
      buffer.size = 0;
    }
  }

  /** Calls `listener` with each new piece of output of `executionId`. Returns the unsubscribe. */
  subscribe(executionId: string, listener: Listener): () => void {
    const set = this.listeners.get(executionId) ?? new Set<Listener>();
    set.add(listener);
    this.listeners.set(executionId, set);
    return () => {
      set.delete(listener);
      if (set.size === 0) this.listeners.delete(executionId);
    };
  }

  private buffer(executionId: string): Buffer {
    let buffer = this.buffers.get(executionId);
    if (!buffer) {
      buffer = { chunks: [], size: 0, nextSeq: 0 };
      this.buffers.set(executionId, buffer);
      for (const old of [...this.buffers.keys()].slice(0, -KEPT_EXECUTIONS)) {
        if (!this.listeners.has(old)) this.buffers.delete(old);
      }
    }
    return buffer;
  }
}

function text(buffer: Buffer): string {
  return buffer.chunks.map((chunk) => chunk.data).join('');
}

/** Adds a chunk and drops the oldest output beyond the limit. */
function append(buffer: Buffer, chunk: Chunk): void {
  buffer.chunks.push({ ...chunk });
  buffer.size += chunk.data.length;
  while (buffer.size > TERMINAL_OUTPUT_LIMIT) {
    const first = buffer.chunks[0];
    if (!first) break;
    const excess = buffer.size - TERMINAL_OUTPUT_LIMIT;
    if (first.data.length <= excess) {
      buffer.chunks.shift();
      buffer.size -= first.data.length;
    } else {
      first.data = first.data.slice(excess);
      buffer.size -= excess;
    }
  }
}
