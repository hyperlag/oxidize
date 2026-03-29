import java.io.*;

public class PrintWriterTest {
    public static void main(String[] args) throws Exception {
        // Write with PrintWriter from path string
        PrintWriter pw = new PrintWriter("test_pw.txt");
        pw.println("Line one");
        pw.println("Line two");
        pw.println("Line three");
        pw.close();

        // Read back known lines with BufferedReader
        BufferedReader br = new BufferedReader(new FileReader("test_pw.txt"));
        System.out.println(br.readLine());
        System.out.println(br.readLine());
        System.out.println(br.readLine());
        br.close();

        // Test PrintWriter from FileWriter
        FileWriter fw = new FileWriter("test_pw2.txt");
        PrintWriter pw2 = new PrintWriter(fw);
        pw2.println("From FileWriter");
        pw2.close();

        BufferedReader br2 = new BufferedReader(new FileReader("test_pw2.txt"));
        System.out.println(br2.readLine());
        br2.close();

        // Clean up
        new File("test_pw.txt").delete();
        new File("test_pw2.txt").delete();
        System.out.println("Done");
    }
}
