impl GraphBuilder {
    fn finish(self) -> (MoveCallGraph, MoveTypeGraph, MoveStateAccessGraph) {
        (
            finish_call_graph(
                self.call_nodes.into_values().collect(),
                self.call_edges.into_values().collect(),
                self.unresolved_calls.into_values().collect(),
            ),
            finish_type_graph(
                self.type_nodes.into_values().collect(),
                self.type_edges.into_values().collect(),
                self.unresolved_types.into_values().collect(),
            ),
            finish_state_access_graph(
                self.state_nodes.into_values().collect(),
                self.state_edges.into_values().collect(),
                self.unresolved_state_accesses.into_values().collect(),
            ),
        )
    }

    fn finish_state_access_graph(self) -> MoveStateAccessGraph {
        finish_state_access_graph(
            self.state_nodes.into_values().collect(),
            self.state_edges.into_values().collect(),
            self.unresolved_state_accesses.into_values().collect(),
        )
    }
}
