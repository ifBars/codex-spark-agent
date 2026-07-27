# Frontier rule-transfer fixture

`frontier-rule-transfer` is the first task in the experimental `frontier`
benchmark suite. It has no reported model score yet and will not enter the
Capability Atlas until difficulty calibration is complete.

The design borrows three evaluation principles:

- ARC-style few-shot generalization: the transformation is specified only by
  demonstrations and must transfer to novel symbolic graphs.
- Agents' Last Exam-style artifact execution: the agent must implement a
  reusable solver, not report a self-graded answer.
- Humanity's Last Exam-style grading: every private case has an unambiguous,
  automatically verifiable result.

Six private, out-of-distribution cases independently score:

| Signal | Weight |
| --- | ---: |
| Amber-pattern distractor transfer | 17 |
| Cyan-pattern tie breaking | 17 |
| Cycle avoidance | 17 |
| Early termination | 17 |
| Edge-weighted choice | 16 |
| Invalid-target resilience | 16 |

Each check also verifies that the solver leaves its input unchanged. The fixture
test proves the starting stub fails and a general reference implementation
passes public and private cases.

The calibration target is deliberately unsaturated: low, medium, and high
reasoning should each remain below roughly 65% over repeated valid runs. This is
a gate, not a forced score. If high reasoning exceeds the target, cases will be
expanded or rotated before publication. Provider/API failures remain exclusions
and ordinary task failures remain scored.

Design references:

- [ARC-AGI overview](https://arcprize.org/arc-agi)
- [Agents' Last Exam](https://agents-last-exam.org/)
- [Humanity's Last Exam paper](https://arxiv.org/abs/2501.14249)
