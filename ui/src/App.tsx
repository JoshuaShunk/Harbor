import { BrowserRouter, Routes, Route, Navigate } from "react-router-dom";
import { UpdateProvider } from "./contexts/UpdateContext";
import Layout from "./components/Layout";
import Servers from "./pages/Servers";
import Hosts from "./pages/Hosts";
import Settings from "./pages/Settings";
import Marketplace from "./pages/Marketplace";

function App() {
  return (
    <UpdateProvider>
      <BrowserRouter>
        <Routes>
          <Route element={<Layout />}>
            <Route path="/" element={<Navigate to="/servers" replace />} />
            <Route path="/servers" element={<Servers />} />
            <Route path="/hosts" element={<Hosts />} />
            <Route path="/marketplace" element={<Marketplace />} />
            <Route path="/settings" element={<Settings />} />
          </Route>
        </Routes>
      </BrowserRouter>
    </UpdateProvider>
  );
}

export default App;
