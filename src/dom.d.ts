import "react";

declare module "react" {
  interface HTMLAttributes<T> {
    /**
     * `inert` is in every browser this app runs in, but not yet in React 18's
     * types. It is load-bearing here: the decorative copies of the navigation
     * and the filter list must be unreachable by Tab and by a screen reader.
     */
    inert?: boolean;
  }
}
