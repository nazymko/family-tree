import type {
  CitationDto,
  EventDto,
  Id,
  MediaDto,
  MediaLinkDto,
  NameDto,
  NoteDto,
  ParentRelId,
  PlaceOption,
  SexKind,
  SourceDto,
  UnionKindId,
} from '../types';

/** In-memory records of the mock database (plain objects so a snapshot is a structuredClone). */
export interface PersonRec {
  id: Id;
  sex: SexKind;
  sexText: string | null;
  livingOverride: 'living' | 'deceased' | null;
  private: boolean;
  names: NameDto[];
  events: EventDto[];
  notes: NoteDto[];
  media: MediaLinkDto[];
  citations: CitationDto[];
  changed: number;
  /** The unnamed-siblings marker: no names, no events, never listed, searched, counted or exported. */
  siblingMarker?: boolean;
}

export interface ChildLinkRec {
  person: Id;
  rel: [ParentRelId, ParentRelId];
  preferred: boolean;
}

export interface FamilyRec {
  id: Id;
  partner1: Id | null;
  partner2: Id | null;
  kind: UnionKindId;
  events: EventDto[];
  children: ChildLinkRec[];
  notes: NoteDto[];
  citations: CitationDto[];
}

export interface Fixture {
  name: string;
  persons: PersonRec[];
  families: FamilyRec[];
  places: PlaceOption[];
  sources: SourceDto[];
  media: MediaDto[];
  homePerson: Id;
  /** Hand-written edge cases; null when the tree is too small to hold them. */
  specials: {
    manyPartners: Id | null;
    cycleA: Id | null;
    cycleB: Id | null;
    undated: Id | null;
  };
}

export interface Db {
  persons: Map<Id, PersonRec>;
  families: Map<Id, FamilyRec>;
  places: Map<Id, PlaceOption>;
  sources: Map<Id, SourceDto>;
  media: Map<Id, MediaDto>;
}

export class MockApiError extends Error {
  constructor(
    public code: 'not_found' | 'invalid' | 'conflict' | 'io' | 'no_project' | 'stale' | 'internal',
    message: string,
  ) {
    super(message);
    this.name = 'MockApiError';
  }
}
