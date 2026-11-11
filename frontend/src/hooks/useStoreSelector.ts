import { useMemo, useSyncExternalStore } from "react";

type Store<State> = {
  subscribe: (listener: () => void) => () => void;
  getSnapshot: () => State;
};

export function shallowEqual<T>(left: T, right: T): boolean {
  if (Object.is(left, right)) return true;
  if (!left || !right || typeof left !== "object" || typeof right !== "object") return false;
  const keys = Object.keys(left);
  return keys.length === Object.keys(right).length && keys.every((key) =>
    Object.prototype.hasOwnProperty.call(right, key) &&
    Object.is(Reflect.get(left, key), Reflect.get(right, key)),
  );
}

/** Cache projections so unrelated store updates do not schedule a consumer render. */
export function useStoreSelector<State, Selected>(
  store: Store<State>,
  select: (state: State) => Selected,
  equal: (left: Selected, right: Selected) => boolean = shallowEqual,
) {
  const getSelection = useMemo(() => {
    let cached: { state: State; selected: Selected } | undefined;
    return () => {
      const state = store.getSnapshot();
      if (cached && Object.is(cached.state, state)) return cached.selected;
      const selected = select(state);
      cached = { state, selected: cached && equal(cached.selected, selected) ? cached.selected : selected };
      return cached.selected;
    };
  }, [store, select, equal]);
  return useSyncExternalStore(store.subscribe, getSelection, getSelection);
}
