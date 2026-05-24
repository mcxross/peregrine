import { createId } from "./ids";
import { sha256Hex } from "./hash";
import type { EvidenceRecord, EvidenceStore } from "./types";
import type { EvidenceCandidate } from "@peregrine/agent-runtime";

export interface EvidencePersistenceAdapter {
  readRecords(): Promise<EvidenceRecord[]>;
  writeRecords(records: EvidenceRecord[]): Promise<void>;
  readBlob(contentHash: string): Promise<string | undefined>;
  writeBlob(contentHash: string, value: string): Promise<void>;
}

export class InMemoryEvidenceStore implements EvidenceStore {
  private readonly records = new Map<string, EvidenceRecord>();

  async record(candidate: EvidenceCandidate): Promise<EvidenceRecord> {
    const rawText = serializeEvidencePayload(candidate.raw ?? candidate.summary);
    const contentHash = candidate.contentHash ?? (await sha256Hex(rawText));
    const record: EvidenceRecord = {
      id: createId("evidence"),
      kind: candidate.kind,
      source: candidate.source,
      summary: candidate.summary,
      rawPath: candidate.rawPath,
      contentHash,
      createdAt: new Date().toISOString(),
      metadata: candidate.metadata,
    };

    this.records.set(record.id, record);

    return record;
  }

  get(id: string) {
    return this.records.get(id);
  }

  list() {
    return Array.from(this.records.values());
  }
}

export class ContentAddressedEvidenceStore implements EvidenceStore {
  private readonly records = new Map<string, EvidenceRecord>();
  private loaded = false;

  constructor(private readonly persistence: EvidencePersistenceAdapter) {}

  async record(candidate: EvidenceCandidate): Promise<EvidenceRecord> {
    await this.load();

    const rawText = serializeEvidencePayload(candidate.raw ?? candidate.summary);
    const contentHash = candidate.contentHash ?? (await sha256Hex(rawText));
    await this.persistence.writeBlob(contentHash, rawText);

    const record: EvidenceRecord = {
      id: createId("evidence"),
      kind: candidate.kind,
      source: candidate.source,
      summary: candidate.summary,
      rawPath: candidate.rawPath ?? `peregrine-evidence://sha256/${contentHash}`,
      contentHash,
      createdAt: new Date().toISOString(),
      metadata: candidate.metadata,
    };

    this.records.set(record.id, record);
    await this.persistence.writeRecords(this.list());

    return record;
  }

  get(id: string) {
    void this.load();
    return this.records.get(id);
  }

  list() {
    return Array.from(this.records.values());
  }

  async rawContent(contentHash: string) {
    return this.persistence.readBlob(contentHash);
  }

  private async load() {
    if (this.loaded) {
      return;
    }

    const records = await this.persistence.readRecords();
    this.records.clear();
    for (const record of records) {
      this.records.set(record.id, record);
    }
    this.loaded = true;
  }
}

export class LocalStorageEvidencePersistence implements EvidencePersistenceAdapter {
  constructor(private readonly namespace = "peregrine:evidence") {}

  async readRecords(): Promise<EvidenceRecord[]> {
    const value = localStorage.getItem(this.recordsKey());
    if (!value) {
      return [];
    }

    try {
      const parsed = JSON.parse(value);
      return Array.isArray(parsed) ? parsed : [];
    } catch {
      return [];
    }
  }

  async writeRecords(records: EvidenceRecord[]): Promise<void> {
    localStorage.setItem(this.recordsKey(), JSON.stringify(records));
  }

  async readBlob(contentHash: string): Promise<string | undefined> {
    return localStorage.getItem(this.blobKey(contentHash)) ?? undefined;
  }

  async writeBlob(contentHash: string, value: string): Promise<void> {
    localStorage.setItem(this.blobKey(contentHash), value);
  }

  private recordsKey() {
    return `${this.namespace}:records`;
  }

  private blobKey(contentHash: string) {
    return `${this.namespace}:blob:${contentHash}`;
  }
}

function serializeEvidencePayload(value: unknown) {
  if (typeof value === "string") {
    return value;
  }

  try {
    return JSON.stringify(value);
  } catch {
    return String(value);
  }
}
