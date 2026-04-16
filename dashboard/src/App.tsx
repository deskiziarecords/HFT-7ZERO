import { useState, useEffect } from 'react';
import {
  Activity,
  ShieldAlert,
  Network,
  Cpu,
  Zap,
  Lock,
  Unlock,
  AlertTriangle,
  ShieldCheck
} from 'lucide-react';
import {
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  AreaChart,
  Area,
  BarChart,
  Bar,
  ReferenceLine
} from 'recharts';

// --- MOCK DATA GENERATOR ---
const generateFrame = () => {
  const timestamp = Date.now();
  const latency = Math.floor(400000 + Math.random() * 200000);
  const status = Math.random() > 0.98 ? 'CLOSED' : 'OPEN';
  const q_t = 26012.8 + (Math.random() - 0.5) * 500;

  return {
    timestamp,
    system_status: status,
    sizing: {
      q_t: q_t.toFixed(1),
      f_kelly: (50.66 + Math.random() * 2).toFixed(2),
      g_vol: (0.79 + Math.random() * 0.05).toFixed(2),
      h_conf: (0.65 + Math.random() * 0.1).toFixed(2)
    },
    routing: [
      { id: 'CME', weight: 0.65, qty: Math.floor(q_t * 0.65) },
      { id: 'LMAX', weight: 0.25, qty: Math.floor(q_t * 0.25) },
      { id: 'ICE', weight: 0.10, qty: Math.floor(q_t * 0.10) }
    ],
    risk: {
      l1: Math.random() > 0.9,
      l2: false,
      l3: false,
      l4: Math.random() > 0.95,
      l5: false,
      l6: false
    },
    performance: {
      latency_ns: latency,
      stealth_score: (0.9990 + Math.random() * 0.0009).toFixed(4)
    }
  };
};

const App = () => {
  const [data, setData] = useState<any>(generateFrame());
  const [history, setHistory] = useState<any[]>([]);

  useEffect(() => {
    const interval = setInterval(() => {
      const frame = generateFrame();
      setData(frame);
      setHistory(prev => [...prev.slice(-49), { ...frame, time: new Date().toLocaleTimeString() }]);
    }, 500);
    return () => clearInterval(interval);
  }, []);

  return (
    <div className="min-h-screen bg-[#0a0a0c] text-gray-100 p-4 font-mono">
      {/* Header */}
      <header className="flex justify-between items-center mb-6 border-b border-gray-800 pb-4">
        <div className="flex items-center gap-3">
          <div className="bg-blue-600 p-2 rounded">
            <Zap className="text-white fill-current" size={24} />
          </div>
          <div>
            <h1 className="text-xl font-bold tracking-tighter">HFT-7ZERO <span className="text-blue-500">STEALTH</span></h1>
            <p className="text-xs text-gray-500 uppercase tracking-widest text-[10px]">Production Node: XC-902 | Hyperion Engine Integrated</p>
          </div>
        </div>

        <div className="flex gap-6">
          <div className="text-right">
            <p className="text-[10px] text-gray-500 uppercase">System Status</p>
            <div className="flex items-center gap-2">
              {data.system_status === 'OPEN' ? (
                <><Unlock size={14} className="text-emerald-500" /><span className="text-emerald-500 font-bold text-sm tracking-widest">LIVE / OPEN</span></>
              ) : (
                <><Lock size={14} className="text-rose-500" /><span className="text-rose-500 font-bold text-sm tracking-widest">LOCKED / ALERT</span></>
              )}
            </div>
          </div>
          <div className="text-right border-l border-gray-800 pl-6">
            <p className="text-[10px] text-gray-500 uppercase">Global PnL (Session)</p>
            <p className="text-emerald-400 font-bold text-sm">+42,805.12</p>
          </div>
        </div>
      </header>

      <main className="grid grid-cols-12 gap-4 h-[calc(100vh-120px)]">
        {/* Left Column: Metrics */}
        <div className="col-span-3 flex flex-col gap-4">
          {/* Confluence Matrix */}
          <section className="bg-[#141417] p-4 rounded-lg border border-gray-800/50 flex-1">
            <div className="flex items-center gap-2 mb-4 border-b border-gray-800 pb-2">
              <Cpu size={16} className="text-blue-500" />
              <h2 className="text-xs font-bold uppercase tracking-wider text-gray-400">EV-ATR Sizing</h2>
            </div>

            <div className="flex flex-col items-center justify-center my-6">
              <p className="text-[10px] text-gray-500 uppercase mb-1 font-bold">Target Q_t</p>
              <p className="text-4xl font-black text-white tracking-tighter tabular-nums">{data.sizing.q_t}</p>
              <p className="text-[10px] text-gray-400 mt-1">Units / Fragment</p>
            </div>

            <div className="space-y-4">
              {[
                { label: 'f_Kelly', val: data.sizing.f_kelly, color: 'text-emerald-400' },
                { label: 'g_Vol', val: data.sizing.g_vol, color: 'text-blue-400' },
                { label: 'h_Conf', val: data.sizing.h_conf, color: 'text-purple-400' },
              ].map(item => (
                <div key={item.label} className="bg-black/30 p-2 rounded border border-gray-800 flex justify-between items-center">
                  <span className="text-[10px] text-gray-500 font-bold italic">#{item.label}</span>
                  <span className={`text-xs font-bold tabular-nums ${item.color}`}>{item.val}</span>
                </div>
              ))}
            </div>
          </section>

          {/* Risk Gate Lambda Grid */}
          <section className="bg-[#141417] p-4 rounded-lg border border-gray-800/50">
            <div className="flex items-center gap-2 mb-4 border-b border-gray-800 pb-2">
              <ShieldAlert size={16} className="text-rose-500" />
              <h2 className="text-xs font-bold uppercase tracking-wider text-gray-400">Risk Gate λ-Telemetry</h2>
            </div>
            <div className="grid grid-cols-3 gap-2">
              {[1, 2, 3, 4, 5, 6].map(i => {
                const triggered = data.risk[`l${i}` as keyof typeof data.risk];
                return (
                  <div key={i} className={`aspect-square flex flex-col items-center justify-center rounded border ${triggered ? 'bg-rose-500/20 border-rose-500' : 'bg-black/40 border-gray-800'}`}>
                    <span className={`text-[10px] font-bold ${triggered ? 'text-rose-500' : 'text-gray-600'}`}>λ{i}</span>
                    <div className={`w-1 h-1 rounded-full mt-1 ${triggered ? 'bg-rose-500 shadow-[0_0_8px_rgba(239,68,68,0.8)] animate-pulse' : 'bg-gray-800'}`}></div>
                  </div>
                );
              })}
            </div>
          </section>
        </div>

        {/* Center Column: Charts */}
        <div className="col-span-6 flex flex-col gap-4">
          {/* Latency 1ms Wall */}
          <section className="bg-[#141417] p-4 rounded-lg border border-gray-800/50 flex-[2]">
            <div className="flex justify-between items-center mb-4 border-b border-gray-800 pb-2">
              <div className="flex items-center gap-2">
                <Activity size={16} className="text-blue-500" />
                <h2 className="text-xs font-bold uppercase tracking-wider text-gray-400">Latency Monitor (1ms Wall)</h2>
              </div>
              <div className="flex gap-4">
                <div className="text-right">
                   <p className="text-[9px] text-gray-500 uppercase">P99</p>
                   <p className="text-xs text-blue-400 font-bold">{(data.performance.latency_ns / 1000).toFixed(1)}μs</p>
                </div>
              </div>
            </div>
            <div className="h-[200px] w-full mt-2">
              <ResponsiveContainer width="100%" height="100%">
                <AreaChart data={history}>
                  <defs>
                    <linearGradient id="colorLat" x1="0" y1="0" x2="0" y2="1">
                      <stop offset="5%" stopColor="#3b82f6" stopOpacity={0.3}/>
                      <stop offset="95%" stopColor="#3b82f6" stopOpacity={0}/>
                    </linearGradient>
                  </defs>
                  <CartesianGrid strokeDasharray="3 3" stroke="#222" vertical={false} />
                  <XAxis dataKey="time" hide />
                  <YAxis domain={[0, 1200000]} hide />
                  <Tooltip contentStyle={{ background: '#141417', border: '1px solid #333', fontSize: '10px' }} />
                  <Area type="monotone" dataKey="performance.latency_ns" stroke="#3b82f6" fillOpacity={1} fill="url(#colorLat)" strokeWidth={2} isAnimationActive={false} />
                  <ReferenceLine y={1000000} stroke="#ef4444" strokeDasharray="5 5" label={{ value: '1ms', fill: '#ef4444', fontSize: 10, position: 'right' }} />
                </AreaChart>
              </ResponsiveContainer>
            </div>
          </section>

          {/* Schur Routing Visualization */}
          <section className="bg-[#141417] p-4 rounded-lg border border-gray-800/50 flex-1">
             <div className="flex items-center gap-2 mb-4 border-b border-gray-800 pb-2">
              <Network size={16} className="text-purple-500" />
              <h2 className="text-xs font-bold uppercase tracking-wider text-gray-400">Schur Optimal Fragmentation</h2>
            </div>
            <div className="h-[100px] w-full">
              <ResponsiveContainer width="100%" height="100%">
                <BarChart layout="vertical" data={data.routing} margin={{ left: -30 }}>
                   <XAxis type="number" hide />
                   <YAxis dataKey="id" type="category" axisLine={false} tickLine={false} tick={{ fontSize: 10, fill: '#666' }} />
                   <Bar dataKey="qty" fill="#8b5cf6" radius={[0, 4, 4, 0]} isAnimationActive={false} />
                </BarChart>
              </ResponsiveContainer>
            </div>
          </section>
        </div>

        {/* Right Column: Order Book & Execution Log */}
        <div className="col-span-3 flex flex-col gap-4">
           {/* Stealth Prob Matrix */}
          <section className="bg-[#141417] p-4 rounded-lg border border-gray-800/50 h-[150px]">
            <div className="flex items-center gap-2 mb-4 border-b border-gray-800 pb-2">
              <ShieldCheck size={16} className="text-emerald-500" />
              <h2 className="text-xs font-bold uppercase tracking-wider text-gray-400">Stealth Coefficient</h2>
            </div>
            <div className="flex flex-col items-center justify-center">
               <p className="text-3xl font-bold text-emerald-500 tabular-nums">{data.performance.stealth_score}</p>
               <p className="text-[10px] text-gray-500 mt-2 font-bold uppercase">Detection Prob: <span className="text-white">0.0012%</span></p>
            </div>
          </section>

          <section className="bg-[#141417] p-4 rounded-lg border border-gray-800/50 flex-1 overflow-hidden">
             <div className="flex items-center gap-2 mb-4 border-b border-gray-800 pb-2">
              <Activity size={16} className="text-gray-500" />
              <h2 className="text-xs font-bold uppercase tracking-wider text-gray-400">Execution Log</h2>
            </div>
            <div className="space-y-2 text-[9px] font-mono opacity-60">
              {history.slice(-10).reverse().map((h, i) => (
                <div key={i} className="flex gap-2">
                  <span className="text-gray-600">[{h.time}]</span>
                  <span className="text-blue-500 uppercase">Routing</span>
                  <span>Q={h.sizing.q_t}</span>
                  <span className="text-emerald-600">ACK</span>
                </div>
              ))}
            </div>
          </section>
        </div>
      </main>

      {/* Footer / Controls */}
      <footer className="fixed bottom-4 left-4 right-4 flex justify-between items-center pointer-events-none">
         <div className="flex gap-2">
            <div className="bg-rose-600 text-white px-4 py-1 rounded text-[10px] font-bold pointer-events-auto cursor-pointer hover:bg-rose-700 flex items-center gap-2">
               <AlertTriangle size={12} /> EMERGENCY SHUTDOWN (PANIC)
            </div>
            <div className="bg-gray-800 text-gray-300 px-4 py-1 rounded text-[10px] font-bold pointer-events-auto cursor-pointer hover:bg-gray-700">
               DRY RUN: OFF
            </div>
         </div>
         <p className="text-[10px] text-gray-700 uppercase font-black italic tracking-widest">Powered by HYPERION TRADE v4.2.0-STABLE</p>
      </footer>
    </div>
  );
};

export default App;
