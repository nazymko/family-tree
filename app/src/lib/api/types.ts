// The wire contract between the UI and the Rust shell. DTOs are generated from Rust by ts-rs into ./generated
// (field names camelCase, enums as in serde); this file re-exports them and keeps only the few types Rust does not yet generate.

import type { ParentRelId, PersonRef, UnionKindId, EventDto, ChildDto } from './generated';

export type * from './generated';

export type Id = string; // UUID v7
export type EventKindId = string; // 'birth' | 'death' | ... | 'custom:<text>' (Rust EventKind, lower_snake)

export interface RecentProject {
  dir: string;
  name: string;
  openedAt: number;
  missing: boolean;
}
/** One family (union) read by id: both partner slots, union type and events, children with their per-parent relationship. */
export interface FamilyDto {
  id: Id;
  partners: [PersonRef | null, PersonRef | null];
  kind: UnionKindId;
  summary: string | null;
  events: EventDto[];
  children: (ChildDto & { preferred: boolean })[];
}
export type { ParentRelId };
