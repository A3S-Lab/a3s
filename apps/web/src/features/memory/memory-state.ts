import type { EvolutionOverview, MemoryForgetSignal, MemoryOverview, MemoryTier } from '../../types/api';

export type MemoryLoadPhase = 'idle' | 'loading' | 'ready' | 'error';
export type MemoryViewMode = 'graph' | 'timeline';
export type MemoryGraphScope = 'balanced' | 'complete';
export type MemoryTimeRange = 'all' | '7d' | '30d' | '90d';
export type MemoryLifecycleFilter = 'llm' | 'consolidated' | 'conflicts';
export type MemorySection = 'memory' | 'evolution';

export type MemoryInspectorSelection = { kind: 'memory'; id: string } | { kind: 'entity'; id: string } | null;

export interface MemoryState {
  memoryPhase: MemoryLoadPhase;
  memoryRefreshing: boolean;
  memoryError: string | null;
  memoryData: MemoryOverview | null;
  memoryLastLoadedAt: number | null;
  memoryView: MemoryViewMode;
  memoryGraphScope: MemoryGraphScope;
  memoryQuery: string;
  memoryTypeFilters: string[];
  memorySourceFilters: string[];
  memoryTierFilters: MemoryTier[];
  memorySignalFilters: MemoryForgetSignal[];
  memoryLifecycleFilters: MemoryLifecycleFilter[];
  memoryTimeRange: MemoryTimeRange;
  memoryInspector: MemoryInspectorSelection;
  memorySection: MemorySection;
  evolutionPhase: MemoryLoadPhase;
  evolutionRefreshing: boolean;
  evolutionError: string | null;
  evolutionData: EvolutionOverview | null;
  evolutionLastLoadedAt: number | null;
  evolutionSelectedId: string | null;
  evolutionBusyId: string | null;
}

export function createMemoryState(): MemoryState {
  return {
    memoryPhase: 'idle',
    memoryRefreshing: false,
    memoryError: null,
    memoryData: null,
    memoryLastLoadedAt: null,
    memoryView: 'graph',
    memoryGraphScope: 'balanced',
    memoryQuery: '',
    memoryTypeFilters: [],
    memorySourceFilters: [],
    memoryTierFilters: [],
    memorySignalFilters: [],
    memoryLifecycleFilters: [],
    memoryTimeRange: 'all',
    memoryInspector: null,
    memorySection: 'memory',
    evolutionPhase: 'idle',
    evolutionRefreshing: false,
    evolutionError: null,
    evolutionData: null,
    evolutionLastLoadedAt: null,
    evolutionSelectedId: null,
    evolutionBusyId: null,
  };
}
