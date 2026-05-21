export const networkOptions = [
  {
    id: "testnet",
    label: "Testnet",
    graphQlUrl: "https://graphql.testnet.sui.io/graphql",
  },
  {
    id: "mainnet",
    label: "Mainnet",
    graphQlUrl: "https://graphql.mainnet.sui.io/graphql",
  },
  {
    id: "devnet",
    label: "Devnet",
    graphQlUrl: "https://graphql.devnet.sui.io/graphql",
  },
  {
    id: "localnet",
    label: "Localnet",
    graphQlUrl: null,
  },
  {
    id: "custom",
    label: "Custom GraphQL",
    graphQlUrl: null,
  },
] as const;

export type NetworkId = string;

export type SuiNetworkSelection = {
  id: NetworkId;
  graphQlUrl?: string | null;
  label?: string | null;
  rpcUrl?: string | null;
  customGraphQlUrl?: string;
};

export const defaultSuiNetworkSelection: SuiNetworkSelection = {
  id: "testnet",
};

export function suiGraphQlUrlForSelection(network: SuiNetworkSelection): string | null {
  if (network.graphQlUrl?.trim()) {
    return network.graphQlUrl.trim();
  }

  if (network.id === "custom") {
    return network.customGraphQlUrl?.trim() || null;
  }

  return networkOptions.find((option) => option.id === network.id)?.graphQlUrl ?? null;
}

export function suiNetworkLabel(network: SuiNetworkSelection): string {
  const option = networkOptions.find((candidate) => candidate.id === network.id) ?? networkOptions[0];

  if (network.label?.trim()) {
    return network.label.trim();
  }

  if (network.id === "custom" && network.customGraphQlUrl) {
    return "Custom GraphQL";
  }

  return option.label;
}

export function suiNetworkSelectionFromEnv(env: {
  alias: string;
  rpc: string;
}): SuiNetworkSelection {
  return {
    graphQlUrl: suiGraphQlUrlForEnv(env),
    id: env.alias,
    label: suiEnvLabel(env.alias),
    rpcUrl: env.rpc,
  };
}

export function suiGraphQlUrlForEnv(env: {
  alias: string;
  rpc: string;
}) {
  const alias = normalizedEnvAlias(env.alias);
  const known = networkOptions.find((option) => normalizedEnvAlias(option.id) === alias);

  if (known?.graphQlUrl) {
    return known.graphQlUrl;
  }

  if (alias === "local") {
    return null;
  }

  if (/graphql\.[^.]+\.sui\.io\/graphql/i.test(env.rpc)) {
    return env.rpc;
  }

  const matchedNetwork = env.rpc.match(/fullnode\.(testnet|mainnet|devnet)\.sui\.io/i)?.[1];
  return matchedNetwork ? `https://graphql.${matchedNetwork}.sui.io/graphql` : null;
}

function suiEnvLabel(alias: string) {
  const normalized = normalizedEnvAlias(alias);

  switch (normalized) {
    case "testnet":
      return "Testnet";
    case "mainnet":
      return "Mainnet";
    case "devnet":
      return "Devnet";
    case "local":
    case "localnet":
      return "Localnet";
    default:
      return alias;
  }
}

function normalizedEnvAlias(alias: string) {
  return alias.trim().toLowerCase();
}
