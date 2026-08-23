import type { ComponentType } from 'react';
import type { IconProps } from '@phosphor-icons/react';
import {
  Brain,
  BracketsCurly,
  Database,
  FlowArrow,
  GitBranch,
  Globe,
  PlugsConnected,
  Robot,
  SignIn,
  SignOut,
  TerminalWindow,
  UserFocus,
  Wrench,
} from '@phosphor-icons/react';
import type { WorkflowStepKind } from './workflow-model';

export const workflowIconByKind: Record<WorkflowStepKind, ComponentType<IconProps>> = {
  input: SignIn,
  output: SignOut,
  transform: BracketsCurly,
  branch: GitBranch,
  human_decision: UserFocus,
  execution: TerminalWindow,
  agent: Robot,
  mcp: PlugsConnected,
  model: Brain,
  tool: Wrench,
  service: Globe,
  memory: Database,
  subworkflow: FlowArrow,
};
