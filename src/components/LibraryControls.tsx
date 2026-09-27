import type { Library } from "../lib/useLibrary";
import { FilterList } from "./FilterList";
import { SearchField } from "./SearchField";
import { ViewControls } from "./ViewControls";

/**
 * The Library's controls, in the sidebar rather than beside the results.
 *
 * The window is usually narrow, and a second column of controls took a third
 * of it. Here they cost nothing: the sidebar is already on screen.
 */
export function LibraryControls({ library }: { library: Library }) {
  return (
    <div className="side__tools">
      <SearchField value={library.query} onChange={library.setQuery} />
      <FilterList
        active={library.filter}
        counts={library.counts}
        onSelect={library.setFilter}
      />
      <ViewControls
        view={library.layout}
        cols={library.cols}
        rows={library.rows}
        onView={library.setLayout}
        onCols={library.setCols}
        onRows={library.setRows}
      />
    </div>
  );
}
