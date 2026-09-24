import { createRootRoute, createRoute, createRouter } from "@tanstack/react-router";
import { HomeView } from "@/home/Home.view";
import { ShellView } from "@/shell/Shell.view";

// The route tree (TanStack Router, code-based). A route renders exactly one feature's View; navigation is the
// router's job and data the ViewModel's. After `skies g feature Products`, add
//   const productsRoute = createRoute({ getParentRoute: () => rootRoute, path: "/products", component: ProductsView });
// and list it in `routeTree`.
const rootRoute = createRootRoute({ component: ShellView });

const homeRoute = createRoute({ getParentRoute: () => rootRoute, path: "/", component: HomeView });

const routeTree = rootRoute.addChildren([homeRoute]);

export const router = createRouter({ routeTree });

// Typed routes: `<Link to>` and `navigate({ to })` accept only the paths this tree declares.
declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}
