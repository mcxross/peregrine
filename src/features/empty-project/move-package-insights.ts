import type {
  CapabilityFinding,
  MovePackage,
  MovePackageSurface,
  ObjectLifecycleFunctionRef,
  ObjectLifecycleMap,
  ObjectLifecycleStage,
  ObjectOwnershipFinding,
  PackageTree,
} from "@/features/empty-project/filesystem-tree";

export type MovePackageInsightsReport = {
  scannerReport: MovePackageScannerReport;
  attackSurface: MovePackageSurface;
};

export type MovePackageScannerReport = {
  packageId: string;
  objects: ObjectScanReport;
  tests: TestsScanReport;
  diagnostics: ScannerDiagnostic[];
};

export type ScannerDiagnostic = {
  scannerId: string;
  severity: "info" | "warning" | "error" | string;
  message: string;
  source: EvidenceSource;
};

export type ScannerConfidence = "high" | "medium" | "low";

export type EvidenceSource =
  | "bytecode"
  | "compiler"
  | "sourceFallback"
  | "scanner"
  | string;

export type ScannerEvidence = {
  source: EvidenceSource;
  confidence: ScannerConfidence;
  message: string;
};

export type ObjectScanReport = {
  capabilityFindings: ObjectScanCapabilityFinding[];
  ownershipFindings: ObjectScanOwnershipFinding[];
  lifecycleMaps: ObjectScanLifecycleMap[];
  sharedObjectStructs: string[];
  diagnostics: ScannerDiagnostic[];
};

export type ObjectScanCapabilityFinding = Omit<CapabilityFinding, "evidence"> & {
  evidence: ScannerEvidence[];
};

export type ObjectScanOwnershipFinding = Omit<ObjectOwnershipFinding, "evidence"> & {
  evidence: ScannerEvidence[];
};

export type ObjectScanLifecycleMap = Omit<
  ObjectLifecycleMap,
  "risks" | "stages" | "touchedBy"
> & {
  stages: ObjectScanLifecycleStage[];
  touchedBy: ObjectScanLifecycleFunctionRef[];
};

export type ObjectScanLifecycleStage = Omit<ObjectLifecycleStage, "evidence" | "functions"> & {
  functions: ObjectScanLifecycleFunctionRef[];
  evidence: ScannerEvidence[];
};

export type ObjectScanLifecycleFunctionRef = Omit<ObjectLifecycleFunctionRef, "evidence"> & {
  evidence: ScannerEvidence[];
};

export type TestsScanReport = {
  hasUnitTests: boolean;
  hasMovyInvariantTests: boolean;
  hasFormalProverSpecs: boolean;
  unitTestCount: number;
  movyInvariantTestCount: number;
  formalProverSpecCount: number;
  unitTests: UnitTestFinding[];
  movyInvariantTests: MovyInvariantFinding[];
  formalProverSpecs: FormalProverSpecFinding[];
  diagnostics: ScannerDiagnostic[];
};

export type UnitTestFinding = {
  moduleName: string;
  functionName: string;
  qualifiedName: string;
  filePath: string;
  sourceFolder: string;
  isRandomTest: boolean;
  expectedFailure: boolean;
  confidence: ScannerConfidence;
  evidence: ScannerEvidence[];
};

export type MovyInvariantFinding = {
  moduleName: string;
  functionName: string;
  qualifiedName: string;
  filePath: string;
  hookKind:
    | "init"
    | "sequencePre"
    | "sequencePost"
    | "functionPre"
    | "functionPost"
    | "oracle"
    | string;
  targetFunction: string | null;
  confidence: ScannerConfidence;
  evidence: ScannerEvidence[];
};

export type FormalProverSpecFinding = {
  specKind: "module" | "function" | "sourceFile" | string;
  moduleName: string;
  functionName: string | null;
  qualifiedName: string;
  filePath: string;
  attributes: string[];
  confidence: ScannerConfidence;
  evidence: ScannerEvidence[];
};

export type MovePackageWire = Omit<MovePackage, "insights" | "surface"> & {
  insights?: MovePackageInsightsReport | null;
  surface?: MovePackageSurface | null;
};

export type PackageTreeWire = Omit<PackageTree, "movePackages"> & {
  movePackages: MovePackageWire[];
};

export function normalizePackageTree(packageTree: PackageTreeWire): PackageTree {
  return {
    ...packageTree,
    movePackages: packageTree.movePackages.map(normalizeMovePackage),
  };
}

function normalizeMovePackage(movePackage: MovePackageWire): MovePackage {
  const attackSurface = movePackage.insights?.attackSurface
    ?? movePackage.surface
    ?? emptyMovePackageSurface();
  const scannerReport = movePackage.insights?.scannerReport
    ?? emptyMovePackageScannerReport(movePackage.name);

  return {
    ...movePackage,
    insights: {
      scannerReport,
      attackSurface,
    },
    surface: attackSurface,
  };
}

function emptyMovePackageScannerReport(packageId: string): MovePackageScannerReport {
  return {
    packageId,
    objects: {
      capabilityFindings: [],
      ownershipFindings: [],
      lifecycleMaps: [],
      sharedObjectStructs: [],
      diagnostics: [],
    },
    tests: {
      hasUnitTests: false,
      hasMovyInvariantTests: false,
      hasFormalProverSpecs: false,
      unitTestCount: 0,
      movyInvariantTestCount: 0,
      formalProverSpecCount: 0,
      unitTests: [],
      movyInvariantTests: [],
      formalProverSpecs: [],
      diagnostics: [],
    },
    diagnostics: [],
  };
}

function emptyMovePackageSurface(): MovePackageSurface {
  return {
    entryFunctionCount: 0,
    capabilityCount: 0,
    sharedObjectCount: 0,
    addressOwnedObjectCount: 0,
    immutableObjectCount: 0,
    wrappedObjectCount: 0,
    partyObjectCount: 0,
    adminControlCount: 0,
    externalCallCount: 0,
    publicPackageRelationshipCount: 0,
    capabilityStructs: [],
    capabilityFindings: [],
    sharedObjectStructs: [],
    objectLifecycleMaps: [],
    objectOwnershipFindings: [],
    adminControlFindings: [],
    externalCallFindings: [],
    publicPackageRelationships: [],
  };
}
