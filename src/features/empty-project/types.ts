export type RecentProjectStatus =
  | {
      kind: "assessed";
      score: number;
      summary: string;
    }
  | {
      kind: "new";
      label: string;
    };

export type RecentProject = {
  id: string;
  name: string;
  path: string;
  status: RecentProjectStatus;
};
