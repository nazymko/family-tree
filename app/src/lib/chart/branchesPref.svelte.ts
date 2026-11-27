/** Where the "show families as branches" choice of the chart built around people is kept on this computer. */
export const BRANCHES_KEY = 'gt.chartBranches';

function stored(): boolean {
  try {
    return localStorage.getItem(BRANCHES_KEY) === '1';
  } catch {
    // Storage can be unavailable; the default (off) applies.
    return false;
  }
}

/** Whether the chart built around people draws every family as a branch of a tree. Off by default. */
class BranchesPref {
  value = $state<boolean>(stored());

  set(on: boolean): void {
    this.value = on;
    try {
      localStorage.setItem(BRANCHES_KEY, on ? '1' : '0');
    } catch {
      // Not remembered after a restart; this session still uses it.
    }
  }

  /** Reads the stored choice again (tests start each case from storage). */
  reload(): void {
    this.value = stored();
  }
}

export const branchesPref = new BranchesPref();
