<p align="center">
   <a href="https://mcxross.xyz/">
     <img src="https://raw.githubusercontent.com/mcxross/peregrine/main/public/peregrine-logo.png" alt="Peregrine logo" width="200" height="200">
   </a>
</p>

<h3 align="center">Peregrine</h3>

<p align="center">
  Peregrine is what you need when code is secondary and understanding behavior is everything
   <br>
</p>

![Peregrine CLI](./assets/peregrine-cli.png)

> [!WARNING]
> **This project is under active development**
>
> Thing are changing rapidly, and the current state of the project may not be stable. Use with caution and expect breaking changes.

## Features

- **Customizable Audit Workflows** — Tailor research and auditing processes to your methodology and requirements
- **Model Agnostic** — Use frontier or open-source models without vendor lock-in
- **Unified Tooling** — Single installation with integrated static analysis and formal verification tooling out of the box
- **TUI/CLI & Desktop** — Run in the terminal, desktop app, or both
- **Built-in Expert Skills** — Specialized capabilities for static analysis, formal verification, and security research tasks
- **Integrated Blockchain Knowledge Base** — Curated blockchain and smart contract knowledge available during analysis
- **Portable Memory** — Preserve context across long-running investigations and research sessions
- **Designed for Long-Running Tasks** — Built to support deep, iterative audits that span multiple sessions
- **Shared Memory** — Shared memory across sessions and other agents to preserve context and avoid redundant analysis

## Quickstart

### Installing and running Peregrine

Run the following on Mac or Linux to install Peregrine:

```shell
curl -fsSL https://mcxross.xyz/peregrine/install.sh | sh
```

Run the following on Windows to install Peregrine:

```powershell
powershell -ExecutionPolicy ByPass -c "irm https://mcxross.xyz/peregrine/install.ps1 | iex"
```

Then simply run `peregrine` to get started.

## Autonomous Audit Flow

Peregrine runs an audit as a coordinator-led investigation. The coordinator keeps the audit plan moving, assigns specialist agents, uses the best available tools for evidence, and only promotes findings that survive adversarial review.

```mermaid
flowchart TD
    classDef core fill:#E8F5EE,stroke:#0F7A4F,color:#161616
    classDef agent fill:#FEF3C7,stroke:#B45309,color:#161616
    classDef tool fill:#FFF7ED,stroke:#C2410C,color:#161616
    classDef evidence fill:#EEF2FF,stroke:#4F46E5,color:#161616
    classDef report fill:#DCFCE7,stroke:#15803D,color:#161616
    classDef gap fill:#FEE2E2,stroke:#B91C1C,color:#161616

    Target["Audit target"]:::core --> Coordinator["Coordinator agent<br/>plans, schedules, and tracks progress"]:::core
    Coordinator --> Understand["Build understanding<br/>code shape, trust boundaries, invariants"]:::core
    Understand --> Hypotheses["Generate attack hypotheses"]:::core

    Hypotheses --> Researcher["Researcher<br/>finds plausible issues"]:::agent
    Researcher --> Tools["Available toolchains<br/>static, graphs, bytecode, fuzzing,<br/>formal checks, knowledge lookup"]:::tool
    Tools --> Evidence["Evidence packets<br/>observations, traces, artifacts, gaps"]:::evidence

    Evidence --> Skeptic["Skeptic<br/>tries to disprove candidates"]:::agent
    Evidence --> Exploiter["Exploiter<br/>tries to build a reproducible attack"]:::agent
    Skeptic --> Judge["Judge<br/>scores evidence and role conclusions"]:::agent
    Exploiter --> Judge

    Judge --> Decision{"Enough evidence?"}:::evidence
    Decision -->|"No"| FollowUp["Refine hypothesis<br/>or record coverage gap"]:::gap
    FollowUp --> Coordinator
    Decision -->|"Yes"| Finding["Evidence-backed finding"]:::report
    Finding --> Report["Final report<br/>confirmed findings and remaining gaps"]:::report
```

## License

    Copyright 2026 McXross

    Licensed under the Apache License, Version 2.0 (the "License");
    you may not use this file except in compliance with the License.
    You may obtain a copy of the License at

       http://www.apache.org/licenses/LICENSE-2.0

    Unless required by applicable law or agreed to in writing, software
    distributed under the License is distributed on an "AS IS" BASIS,
    WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
    See the License for the specific language governing permissions and
    limitations under the License.
