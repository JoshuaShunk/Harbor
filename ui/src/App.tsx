import { BrowserRouter, Routes, Route, Navigate } from "react-router-dom";
import { UpdateProvider } from "./contexts/UpdateContext";
import { ThemeProvider } from "./contexts/ThemeContext";
import { LogProvider } from "./contexts/LogContext";
import Layout from "./components/Layout";
import Servers from "./pages/Servers";
import Hosts from "./pages/Hosts";
import Settings from "./pages/Settings";
import Marketplace from "./pages/Marketplace";
import Lighthouse from "./pages/Lighthouse";

function App() {
  return (
    <ThemeProvider>
    <UpdateProvider>
    <LogProvider>
      <BrowserRouter>
        <Routes>
          <Route element={<Layout />}>
            <Route path="/" element={<Navigate to="/servers" replace />} />
            <Route path="/servers" element={<Servers />} />
            <Route path="/hosts" element={<Hosts />} />
            <Route path="/marketplace" element={<Marketplace />} />
            <Route path="/lighthouse" element={<Lighthouse />} />
            <Route path="/settings" element={<Settings />} />
          </Route>
        </Routes>
      </BrowserRouter>
    </LogProvider>
    </UpdateProvider>
    </ThemeProvider>
  );
}

export default App;
