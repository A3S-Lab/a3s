import {
  CheckCircle2,
  CircuitBoard,
  ClipboardCheck,
  Cpu,
  FileKey2,
  LoaderCircle,
  LockKeyhole,
  ShieldAlert,
} from 'lucide-react';
import { useSnapshot } from 'valtio';
import { shellDisplayArgument } from '../../../lib/bench-command';
import { isEvaluationActive, labState } from '../../../state/lab-state';
import type { EngineeringController } from '../use-engineering-controller';

interface EngineeringWorkspaceProps {
  actions: EngineeringController;
}

export function EngineeringWorkspace({ actions }: EngineeringWorkspaceProps) {
  const state = useSnapshot(labState);
  const live = state.connection.mode === 'live';
  const busy = state.engineering.activeOperation;
  const evaluationActive = isEvaluationActive(state.run.stage, state.campaign.status, state.run.trackingStatus);
  const writeLockReasonId = evaluationActive ? 'engineering-write-lock-reason' : undefined;

  return (
    <main className='workspace-page engineering-workspace'>
      <header className='workspace-page-header engineering-command-header'>
        <div>
          <p className='workspace-eyebrow'>SYSTEM BAY / ADVANCED MODULES</p>
          <h1>工程舱</h1>
          <p>维护评测航电、扫描任务模块，并封装可复现的 Task 与 Candidate Lock。</p>
        </div>
        <span className={`environment-badge ${live ? 'is-live' : ''}`}>
          <i /> {live ? (evaluationActive ? '评测中 · 工程写入锁定' : '工程链路在线') : '模拟舱 · 文件写入锁定'}
        </span>
      </header>

      <section className='engineering-grid' aria-label='工程舱模块'>
        {evaluationActive ? (
          <p className='sr-only' id='engineering-write-lock-reason'>
            评测运行中，Lock 文件写入与出击配置同步已锁定。
          </p>
        ) : null}
        <section className='engineering-card doctor-card'>
          <div className='engineering-card-icon'>
            <CircuitBoard size={19} />
          </div>
          <div className='engineering-card-copy'>
            <span className='card-index'>MODULE 01 / AVIONICS</span>
            <h2>航电自检</h2>
            <p>扫描 Bench 配置、Runtime Provider 与 Judge 模型路由，确认整套评测链路可用。</p>
          </div>
          <div className='runtime-readout'>
            <span>DOCTOR READOUT</span>
            <dl>
              <div>
                <dt>Runtime Provider</dt>
                <dd>{state.connection.doctor?.runtime.provider ?? '待自检'}</dd>
              </div>
              <div>
                <dt>Runtime 版本 / 详情</dt>
                <dd>{state.connection.doctor?.runtime.detail ?? state.connection.message}</dd>
              </div>
              <div>
                <dt>Judge Model</dt>
                <dd>{state.connection.doctor ? state.connection.doctor.judge_model?.trim() || '未配置' : '待自检'}</dd>
              </div>
            </dl>
          </div>
          <button
            className='secondary-action'
            onClick={() => void actions.runDoctor()}
            disabled={!live || Boolean(busy)}
          >
            {busy === 'doctor' ? <LoaderCircle className='spin' size={15} /> : <Cpu size={15} />}
            {state.connection.doctor ? '重新自检' : '启动自检'}
          </button>
        </section>

        <section className='engineering-card validation-card'>
          <div className='engineering-card-icon'>
            <ClipboardCheck size={19} />
          </div>
          <div className='engineering-card-copy'>
            <span className='card-index'>MODULE 02 / SCANNER</span>
            <h2>任务模块校验</h2>
            <p>装载 `task.acl`、Judge Asset 与任务约束，执行只读扫描，不生成锁或战报。</p>
          </div>
          <label className='stacked-field'>
            <span>任务模块路径</span>
            <input
              value={state.engineering.taskSource}
              onChange={(event) => actions.setTaskSource(event.target.value)}
              placeholder='./task'
            />
          </label>
          <details className='command-disclosure'>
            <summary>展开高级指令</summary>
            <code>a3s bench advanced check {shellDisplayArgument(state.engineering.taskSource || './task')}</code>
          </details>
          <button className='primary-action' onClick={() => void actions.checkTask()} disabled={!live || Boolean(busy)}>
            {busy === 'check' ? <LoaderCircle className='spin' size={15} /> : <ClipboardCheck size={15} />}
            扫描任务模块
          </button>
        </section>

        <section className='engineering-card task-lock-card'>
          <div className='engineering-card-icon'>
            <FileKey2 size={19} />
          </div>
          <div className='engineering-card-copy'>
            <span className='card-index'>MODULE 03 / MISSION FORGE</span>
            <h2>任务封装舱</h2>
            <p>将内置任务或本地 TaskBundle 与 Judge、镜像及任务修订封装为 Task Lock。</p>
          </div>
          <div className='field-pair'>
            <label className='stacked-field'>
              <span>Task 来源</span>
              <input
                value={state.engineering.taskSource}
                onChange={(event) => actions.setTaskSource(event.target.value)}
                placeholder='quick_file_edit 或 ./task'
              />
            </label>
            <label className='stacked-field'>
              <span>封装输出</span>
              <input
                value={state.engineering.taskLockOutput}
                onChange={(event) => actions.setTaskLockOutput(event.target.value)}
                placeholder='./task.lock.json'
              />
            </label>
          </div>
          <details className='command-disclosure'>
            <summary>展开高级指令</summary>
            <code>
              a3s bench advanced task lock {shellDisplayArgument(state.engineering.taskSource || './task')} --out{' '}
              {shellDisplayArgument(state.engineering.taskLockOutput || './task.lock.json')}
            </code>
          </details>
          <button
            className='primary-action'
            onClick={() => void actions.createTaskLock()}
            disabled={!live || Boolean(busy) || evaluationActive}
            aria-describedby={writeLockReasonId}
          >
            {busy === 'task-lock' ? <LoaderCircle className='spin' size={15} /> : <LockKeyhole size={15} />}
            封装 Task Lock
          </button>
        </section>

        <section className='engineering-card candidate-lock-card'>
          <div className='engineering-card-icon'>
            <LockKeyhole size={19} />
          </div>
          <div className='engineering-card-copy'>
            <span className='card-index'>MODULE 04 / AGENT FORGE</span>
            <h2>智能体封装舱</h2>
            <p>绑定 Candidate 适配器与可选模型核心，为锁定模式生成 Candidate Lock。</p>
          </div>
          <div className='field-pair field-pair-three'>
            <label className='stacked-field'>
              <span>智能体适配器</span>
              <input
                value={state.engineering.candidate}
                onChange={(event) => actions.setCandidate(event.target.value)}
                placeholder='./candidate'
              />
            </label>
            <label className='stacked-field'>
              <span>模型核心（可选）</span>
              <input
                value={state.engineering.candidateModel}
                onChange={(event) => actions.setCandidateModel(event.target.value)}
                placeholder='provider/model'
              />
            </label>
            <label className='stacked-field'>
              <span>封装输出</span>
              <input
                value={state.engineering.candidateLockOutput}
                onChange={(event) => actions.setCandidateLockOutput(event.target.value)}
                placeholder='./candidate.lock.json'
              />
            </label>
          </div>
          <details className='command-disclosure'>
            <summary>展开高级指令</summary>
            <code>
              a3s bench advanced candidate lock {shellDisplayArgument(state.engineering.candidate || './candidate')}
              {state.engineering.candidateModel
                ? ` --model ${shellDisplayArgument(state.engineering.candidateModel)}`
                : ''}{' '}
              --out {shellDisplayArgument(state.engineering.candidateLockOutput || './candidate.lock.json')}
            </code>
          </details>
          <button
            className='primary-action'
            onClick={() => void actions.createCandidateLock()}
            disabled={!live || Boolean(busy) || evaluationActive}
            aria-describedby={writeLockReasonId}
          >
            {busy === 'candidate-lock' ? <LoaderCircle className='spin' size={15} /> : <LockKeyhole size={15} />}
            封装 Candidate Lock
          </button>
        </section>
      </section>

      <output
        className={`operation-status ${state.engineering.error ? 'is-error' : state.engineering.lastOperation ? 'is-success' : ''}`}
      >
        {state.engineering.error ? <ShieldAlert size={15} /> : <CheckCircle2 size={15} />}
        <span>
          {state.engineering.error ??
            state.engineering.lastOperation?.message ??
            '工程舱模块只在本机执行；所有路径均相对于 Bridge 的 Bench 工作目录解析。'}
        </span>
      </output>
    </main>
  );
}
