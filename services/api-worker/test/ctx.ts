// worker.fetch の第 3 引数(ExecutionContext)の偽物。waitUntil の Promise は待たずに失敗だけ握りつぶす。
export function fakeCtx(): ExecutionContext {
  return {
    waitUntil: (p: Promise<unknown>) => { void p.catch(() => {}); },
    passThroughOnException: () => {},
    props: {},
  } as unknown as ExecutionContext;
}
