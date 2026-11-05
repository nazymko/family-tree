// The wire contract between the UI and the Rust shell. DTOs are generated from Rust by ts-rs into ./generated
// (field names camelCase, enums as in serde); this file re-exports them and adds Id aliases.

import type { ParentRelId } from './generated';

export type * from './generated';

export type Id = string; // UUID v7
export type EventKindId = string; // 'birth' | 'death' | ... | 'custom:<text>' (Rust EventKind, lower_snake)

export type { ParentRelId };

// --- Agent access (hand-written; the Rust side implements the commands to match) ---
/** One agent connection. The secret key is never part of it: it is shown once, when the connection is created. */
export interface ConnectionDto {
  id: string;
  name: string;
  /** Unix time in milliseconds. */
  created: number;
}

export interface AgentAccessState {
  enabled: boolean;
  port: number;
  running: boolean;
  /** Plain words, for example when the port is taken; null when all is well. */
  error: string | null;
  connections: ConnectionDto[];
}

export interface NewConnectionDto {
  connection: ConnectionDto;
  /** The plain secret key. Shown once and never returned again. */
  key: string;
  /** A ready command that connects an agent, containing the key. */
  snippet: string;
}
// --- end of Agent access ---
