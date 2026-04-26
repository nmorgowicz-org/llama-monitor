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
        listener.Prefixes.Add("http://127.0.0.1:7780/");
        listener.Start();

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
