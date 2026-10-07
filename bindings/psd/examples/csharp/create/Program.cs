using PhotoCraft.Psd;

if (args.Length != 1) { Console.Error.WriteLine("usage: create <output.psd>"); return 1; }
try
{
    using var builder = new Builder(1, 1);
    byte[] pixels = [255, 0, 0, 128];
    builder.AddLayer("Red", new(0, 0, 1, 1), pixels);
    builder.Composite(pixels);
    builder.Resolution(300);
    using var doc = builder.Build();
    File.WriteAllBytes(args[0], doc.ToBytes());
}
catch (Exception error)
{
    Console.Error.WriteLine(error.Message); return 1;
}
return 0;
