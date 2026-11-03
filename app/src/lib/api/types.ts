// The wire contract between the UI and the Rust shell. DTOs are generated from Rust by ts-rs into ./generated
// (field names camelCase, enums as in serde); this file re-exports them and adds Id aliases.

import type { ParentRelId } from './generated';

export type * from './generated';

export type Id = string; // UUID v7
export type EventKindId = string; // 'birth' | 'death' | ... | 'custom:<text>' (Rust EventKind, lower_snake)

export type { ParentRelId };
