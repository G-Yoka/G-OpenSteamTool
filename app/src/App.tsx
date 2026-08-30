import { AppShell } from "./app/AppShell";
import { useAppController } from "./app/useAppController";

export default function App() {
  return <AppShell controller={useAppController()} />;
}
