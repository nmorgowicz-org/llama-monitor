using LibreHardwareMonitor.Hardware;
using System;
using System.Collections.Generic;
using System.Net;
using System.Text;
using System.Text.Json;
using System.Threading;

class UpdateVisitor : IVisitor
{
    public void VisitComputer(IComputer computer) { computer.Traverse(this); }
    public void VisitHardware(IHardware hardware)
    {
        hardware.Update();
        foreach (var sub in hardware.SubHardware) sub.Accept(this);
    }
    public void VisitSensor(ISensor sensor) { }
    public void VisitParameter(IParameter parameter) { }
}

class Program
{
    static void Main(string[] args)
    {
        var computer = new Computer
        {
            IsCpuEnabled = true,
            IsMotherboardEnabled = true,
            IsMemoryEnabled = false,
            IsGpuEnabled = false,
            IsStorageEnabled = false,
        };
        computer.Open();

        string lastJson = CollectSensors(computer);

        var timer = new Timer(_ =>
        {
            try { lastJson = CollectSensors(computer); }
            catch { }
        }, null, TimeSpan.FromSeconds(5), TimeSpan.FromSeconds(5));

       var listener = new HttpListener();
        const int port = 7780;
        const string prefix = "http://127.0.0.1:7780/";
        const uint urlConflictError = 0x80070032;

        // Try to start, cleaning up stale reservations or conflicting processes
        for (int attempt = 0; attempt < 3; attempt++)
        {
            try
            {
                listener.Prefixes.Add(prefix);
                listener.Start();
                break;
            }
            catch (HttpListenerException ex) when (ex.NativeErrorCode == urlConflictError) // URL conflict
            {
                // Attempt 1: delete stale URL reservation
                try
                {
                    var proc = System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo
                    {
                        FileName = "netsh",
                        Arguments = $"http delete urlacl url={prefix}",
                        UseShellExecute = false,
                        CreateNoWindow = true,
                        RedirectStandardOutput = true,
                        RedirectStandardError = true,
                    });
                    proc?.WaitForExit(2000);
                }
                catch { }

                // Attempt 2: kill any existing sensor_bridge processes
                try
                {
                    foreach (var p in System.Diagnostics.Process.GetProcessesByName("sensor_bridge"))
                    {
                        if (p.Id != Environment.ProcessId)
                        {
                            p.Kill();
                            p.WaitForExit(2000);
                        }
                    }
                }
                catch { }

                Thread.Sleep(500); // Let HTTP.sys settle
            }
        }

        Console.WriteLine($"sensor_bridge: listening on {prefix}");

        while (true)
        {
            try
            {
                var ctx = listener.GetContext();
                var bytes = Encoding.UTF8.GetBytes(lastJson);
                ctx.Response.ContentType = "application/json";
                ctx.Response.ContentLength64 = bytes.Length;
                ctx.Response.OutputStream.Write(bytes, 0, bytes.Length);
                ctx.Response.Close();
            }
            catch { }
        }
    }

    static string CollectSensors(Computer computer)
    {
        computer.Accept(new UpdateVisitor());
        var sensors = new List<object>();
        foreach (var hardware in computer.Hardware)
        {
            foreach (var subHardware in hardware.SubHardware)
                foreach (var sensor in subHardware.Sensors)
                    sensors.Add(new { hardware = hardware.Name, subhardware = subHardware.Name, name = sensor.Name, type = sensor.SensorType.ToString(), value = sensor.Value });
            foreach (var sensor in hardware.Sensors)
                sensors.Add(new { hardware = hardware.Name, subhardware = (string?)null, name = sensor.Name, type = sensor.SensorType.ToString(), value = sensor.Value });
        }
        return JsonSerializer.Serialize(sensors);
    }
}
