import {
  Boxes,
  ChevronDown,
  ChevronRight,
  CloudLightning,
  CloudRain,
  CloudSun,
  Crosshair,
  Filter,
  MapPinned,
  Search,
  ShieldCheck,
  ShieldQuestion,
  Snowflake,
  Target,
  Wind,
} from 'lucide-react';
import { useEffect, useMemo, useRef } from 'react';
import { useSnapshot } from 'valtio';
import { type BenchController, filteredTasks } from '../features/bench/use-bench-controller';
import { isEvaluationActive, labState } from '../state/lab-state';
import type { BenchTask } from '../types/bench';
import { taskTheater } from './scene/battlefield-theater';
import { taskWeather, type WeatherId } from './scene/task-weather';
import { TaskRunControls } from './task-run-controls';

interface TaskCatalogProps {
  actions: BenchController;
}

export function TaskCatalog({ actions }: TaskCatalogProps) {
  const state = useSnapshot(labState);
  const searchRef = useRef<HTMLInputElement>(null);
  const categories = useMemo(
    () => [...new Set(state.catalog.tasks.map((task) => task.category))].sort(),
    [state.catalog.tasks]
  );
  const tasks = filteredTasks(
    state.catalog.tasks as readonly BenchTask[],
    state.catalog.query,
    state.catalog.category,
    state.catalog.includeBlocked
  );
  const readyCount = state.catalog.tasks.filter((task) => task.availability === 'ready').length;
  const selectedTask = state.catalog.tasks.find((task) => task.id === state.catalog.selectedTaskId);
  const runActive = isEvaluationActive(state.run.stage, state.campaign.status);

  useEffect(() => {
    const focusSearch = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLocaleLowerCase() === 'k') {
        event.preventDefault();
        searchRef.current?.focus();
        searchRef.current?.select();
      }
    };
    document.addEventListener('keydown', focusSearch);
    return () => document.removeEventListener('keydown', focusSearch);
  }, []);

  return (
    <aside className='task-catalog' aria-label='评测地图选择'>
      <div className='panel-heading mission-heading'>
        <div>
          <h2>选择作战地图</h2>
          <p>地图决定任务目标与战场环境</p>
        </div>
        <output className='mission-ready-count' aria-label={`${readyCount} 个地图可部署`}>
          <span>{String(readyCount).padStart(2, '0')}</span>
          <small>可部署</small>
        </output>
      </div>

      <details className='mission-filters'>
        <summary>
          <span>
            <Filter size={12} aria-hidden='true' />
            地图筛选
          </span>
          <output>
            {tasks.length} / {state.catalog.tasks.length}
          </output>
          <ChevronDown size={13} aria-hidden='true' />
        </summary>
        <div className='mission-filter-panel'>
          <label className='search-control'>
            <Search size={14} aria-hidden='true' />
            <span className='sr-only'>搜索地图或任务</span>
            <input
              ref={searchRef}
              value={state.catalog.query}
              onChange={(event) => actions.setQuery(event.target.value)}
              placeholder='地图、目标或类别…'
            />
            <kbd>⌘ K</kbd>
          </label>

          <div className='catalog-filters'>
            <label className='category-select'>
              <MapPinned size={12} aria-hidden='true' />
              <span className='sr-only'>地图区域</span>
              <select value={state.catalog.category} onChange={(event) => actions.setCategory(event.target.value)}>
                <option value='all'>全部战区</option>
                {categories.map((category) => (
                  <option value={category} key={category}>
                    {category}
                  </option>
                ))}
              </select>
            </label>
            <label className='blocked-toggle'>
              <input
                type='checkbox'
                checked={state.catalog.includeBlocked}
                onChange={(event) => actions.setIncludeBlocked(event.target.checked)}
              />
              <span>封锁区</span>
            </label>
          </div>
        </div>
      </details>

      <div className='mission-board'>
        <div className='mission-route-heading'>
          <span>
            <MapPinned size={12} aria-hidden='true' /> 战区关卡
          </span>
          <small>选择后同步三维战场</small>
        </div>

        <section className='mission-map-list' aria-label='可选任务地图'>
          {tasks.map((task, index) => (
            <MissionMapCard
              task={task}
              index={index}
              selected={task.id === state.catalog.selectedTaskId}
              onSelect={() => void actions.selectTask(task.id)}
              disabled={runActive}
              key={task.id}
            />
          ))}
          {!tasks.length ? (
            <div className='catalog-empty'>
              <Boxes size={24} aria-hidden='true' />
              <strong>没有匹配地图</strong>
              <span>展开情报筛选，调整战区或显示封锁区。</span>
            </div>
          ) : null}
        </section>

        {selectedTask ? <SelectedMission task={selectedTask as BenchTask} actions={actions} /> : null}
      </div>
    </aside>
  );
}

interface SelectedMissionProps {
  task: BenchTask;
  actions: BenchController;
}

function SelectedMission({ task, actions }: SelectedMissionProps) {
  const weather = taskWeather(task.id);
  const theater = taskTheater(task.id, task.category);
  const difficulty = missionDifficulty(task);

  return (
    <section
      className={`selected-mission weather-${weather.id}`}
      aria-label='当前地图简报'
      data-theater={theater.id}
      style={
        {
          '--mission-sky': colorFromNumber(weather.skyColor),
          '--theater-ground': colorFromNumber(theater.palette.ground),
          '--theater-accent': colorFromNumber(theater.palette.accent),
        } as React.CSSProperties
      }
    >
      <div className='mission-visual' aria-hidden='true'>
        <span className='mission-grid' />
        <span className='mission-route route-one' />
        <span className='mission-route route-two' />
        <span className='mission-target-pulse'>
          <Crosshair size={19} />
        </span>
        <span className='mission-weather-mark'>{weatherIcon(weather.id, 28)}</span>
        <small>
          {theater.labelZh} · 区域 {shortSector(task.id)}
        </small>
      </div>

      <div className='selected-mission-copy'>
        <div className='mission-eyebrow'>
          <span>已选地图 · 区域 {shortSector(task.id)}</span>
        </div>
        <h3>{task.name}</h3>
        <div className='mission-meta'>
          <span>
            {weatherIcon(weather.id, 12)} {weather.labelZh}
          </span>
          <span className={`difficulty-${difficulty.tone}`}>{difficulty.label}</span>
          <span>{theater.labelZh}</span>
        </div>
        <div className='mission-objective'>
          <Target size={13} aria-hidden='true' />
          <p>{task.description ?? availabilityDescription(task.availability_reason)}</p>
        </div>
      </div>

      <TaskRunControls actions={actions} task={task} />

      <details className='mission-intel'>
        <summary>地图情报与准入规则</summary>
        <dl>
          <div>
            <dt>任务 ID</dt>
            <dd>{task.id}</dd>
          </div>
          <div>
            <dt>可用性</dt>
            <dd>{task.availability_reason}</dd>
          </div>
          <div>
            <dt>准入</dt>
            <dd>{task.admission_reason}</dd>
          </div>
          <div>
            <dt>来源</dt>
            <dd>{task.provenance_ref}</dd>
          </div>
        </dl>
      </details>
    </section>
  );
}

interface MissionMapCardProps {
  task: BenchTask;
  index: number;
  selected: boolean;
  onSelect: () => void;
  disabled?: boolean;
}

function MissionMapCard({ task, index, selected, onSelect, disabled = false }: MissionMapCardProps) {
  const weather = taskWeather(task.id);
  const theater = taskTheater(task.id, task.category);
  const difficulty = missionDifficulty(task);

  return (
    <button
      className={`mission-map-card ${selected ? 'is-selected' : ''} ${
        task.availability === 'blocked' ? 'is-blocked' : ''
      }`}
      onClick={onSelect}
      disabled={disabled}
      aria-current={selected ? 'true' : undefined}
      aria-label={`选择地图 ${task.name}，${theater.labelZh}，${weather.labelZh}，${difficulty.label}`}
      data-theater={theater.id}
      style={
        {
          '--mission-sky': colorFromNumber(weather.skyColor),
          '--theater-ground': colorFromNumber(theater.palette.ground),
          '--theater-ground-secondary': colorFromNumber(theater.palette.groundSecondary),
          '--theater-accent': colorFromNumber(theater.palette.accent),
          '--theater-water': colorFromNumber(theater.palette.water),
        } as React.CSSProperties
      }
    >
      <span className='map-thumbnail' aria-hidden='true'>
        <i />
        {weatherIcon(weather.id, 19)}
        <small>{String(index + 1).padStart(2, '0')}</small>
      </span>
      <span className='map-card-copy'>
        <small>
          {theater.labelZh} · {weather.labelZh}
        </small>
        <strong>{task.name}</strong>
        <span className='map-card-status'>
          <span className={`difficulty-${difficulty.tone}`}>{difficulty.label}</span>
          <span className={task.admission === 'admitted' ? 'tag-admitted' : 'tag-quarantined'}>
            {task.admission === 'admitted' ? <ShieldCheck size={10} /> : <ShieldQuestion size={10} />}
            {task.admission === 'admitted' ? '已准入' : '隔离评测'}
          </span>
        </span>
      </span>
      <ChevronRight size={14} aria-hidden='true' />
    </button>
  );
}

export function missionDifficulty(task: BenchTask): { label: string; tone: 'low' | 'medium' | 'high' | 'locked' } {
  if (task.availability === 'blocked') return { label: '封锁', tone: 'locked' };
  if (task.execution_class === 'conformance') return { label: '训练', tone: 'low' };
  if (task.admission === 'admitted') return { label: '标准', tone: 'medium' };
  return { label: '高危', tone: 'high' };
}

function availabilityDescription(reason: string): string {
  if (reason === 'bundled_offline_task') return '内置离线一致性任务，可用于快速验证 Candidate 适配器。';
  if (reason === 'bundled_oci_task') return '内置 OCI 长时任务，运行前会解析并锁定所需镜像。';
  return '该任务的详细描述由本机 A3S Bench 目录提供。';
}

function shortSector(taskId: string): string {
  let hash = 0;
  for (const character of taskId) hash = (hash * 31 + character.charCodeAt(0)) >>> 0;
  return `${String.fromCharCode(65 + (hash % 6))}-${String((hash % 89) + 10).padStart(2, '0')}`;
}

function colorFromNumber(value: number): string {
  return `#${value.toString(16).padStart(6, '0')}`;
}

function weatherIcon(weatherId: WeatherId, size: number): React.ReactNode {
  if (weatherId === 'clear') return <CloudSun size={size} aria-hidden='true' />;
  if (weatherId === 'hail') return <Snowflake size={size} aria-hidden='true' />;
  if (weatherId === 'typhoon') return <Wind size={size} aria-hidden='true' />;
  if (weatherId === 'thunderstorm' || weatherId === 'mixed') {
    return <CloudLightning size={size} aria-hidden='true' />;
  }
  return <CloudRain size={size} aria-hidden='true' />;
}
