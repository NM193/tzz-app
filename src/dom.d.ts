import "react";

declare module "react" {
  interface HTMLAttributes<T> {
    /**
     * `inert` is in every browser this app runs in, but not yet in React 18's
     * types. It is load-bearing here: the decorative copies of the navigation
     * and the filter list must be unreachable by Tab and by a screen reader.
     *
     * React 18 drops a boolean `inert`, with a warning, and writes only a
     * string -- so it is `inert=""`, never `inert` or `inert={true}`. React 19
     * knows it as a boolean and reads "" as false: moving to 19 means turning
     * these back into booleans and deleting this declaration.
     */
    inert?: "";
  }
}
