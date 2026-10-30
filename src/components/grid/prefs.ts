// How every grid looks (Settings › Tabs & data grid), provided once by the app.

import { createContext, useContext } from "react";

export interface GridPrefs {
  rowHeight: number;
  stripes: boolean;
  rowNumbers: boolean;
}

export const ROW_HEIGHTS = { compact: 20, normal: 24, comfortable: 30 } as const;

export const GridPrefsContext = createContext<GridPrefs>({
  rowHeight: ROW_HEIGHTS.normal,
  stripes: true,
  rowNumbers: true,
});

export const useGridPrefs = () => useContext(GridPrefsContext);
