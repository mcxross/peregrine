import { createId } from "./ids";
import { sha256Hex } from "./hash";
import type { EvidenceRecord, EvidenceStore } from "./types";
import type { EvidenceCandidate } from "@peregrine/agent-runtime";

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

