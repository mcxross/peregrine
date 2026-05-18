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

export type NetworkId = (typeof networkOptions)[number]["id"];

export type SuiNetworkSelection = {
  id: NetworkId;
  customGraphQlUrl?: string;
};

export const defaultSuiNetworkSelection: SuiNetworkSelection = {
  id: "testnet",
};

export function suiGraphQlUrlForSelection(network: SuiNetworkSelection): string | null {
  if (network.id === "custom") {
    return network.customGraphQlUrl?.trim() || null;
  }

  return networkOptions.find((option) => option.id === network.id)?.graphQlUrl ?? null;
}

export function suiNetworkLabel(network: SuiNetworkSelection): string {
  const option = networkOptions.find((candidate) => candidate.id === network.id) ?? networkOptions[0];

  if (network.id === "custom" && network.customGraphQlUrl) {
    return "Custom GraphQL";
  }

  return option.label;
}
