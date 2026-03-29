import java.io.*;

public class BufferedReaderWriter {
    public static void main(String[] args) throws Exception {
        // Write lines with BufferedWriter
        FileWriter fw = new FileWriter("test_brw.txt");
        BufferedWriter bw = new BufferedWriter(fw);
        bw.write("Hello from BufferedWriter");
        bw.newLine();
        bw.write("Second line");
        bw.newLine();
        bw.write("Third line");
        bw.newLine();
        bw.close();

        // Read known number of lines with BufferedReader
        FileReader fr = new FileReader("test_brw.txt");
        BufferedReader br = new BufferedReader(fr);
        System.out.println(br.readLine());
        System.out.println(br.readLine());
        System.out.println(br.readLine());
        br.close();

        // Verify file exists then clean up
        File f = new File("test_brw.txt");
        System.out.println("Exists: " + f.exists());
        f.delete();
        System.out.println("Deleted: " + !f.exists());
    }
}
