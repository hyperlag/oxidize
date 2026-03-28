/**
 * A simple CSV parser and processor. Exercises: string manipulation,
 * ArrayList, iteration, OOP (multiple classes), StringBuilder, and
 * control flow.
 */
public class CsvParser {
    private String data;

    public CsvParser(String data) {
        this.data = data;
    }

    public int countRows() {
        if (data.isEmpty()) {
            return 0;
        }
        int count = 1;
        for (int i = 0; i < data.length(); i++) {
            if (data.charAt(i) == '\n') {
                count++;
            }
        }
        // Don't count trailing newline as a row
        if (data.charAt(data.length() - 1) == '\n') {
            count--;
        }
        return count;
    }

    public int countCols() {
        int count = 1;
        for (int i = 0; i < data.length(); i++) {
            char c = data.charAt(i);
            if (c == ',') {
                count++;
                break;  // Count commas in first row only
            }
            if (c == '\n') {
                break;
            }
        }
        // Actually count all commas in the first row
        count = 1;
        for (int i = 0; i < data.length(); i++) {
            char c = data.charAt(i);
            if (c == '\n') {
                break;
            }
            if (c == ',') {
                count++;
            }
        }
        return count;
    }

    public String getCell(int row, int col) {
        int currentRow = 0;
        int currentCol = 0;
        int start = 0;

        for (int i = 0; i <= data.length(); i++) {
            boolean atEnd = (i == data.length());
            boolean atSep = !atEnd && (data.charAt(i) == ',' || data.charAt(i) == '\n');

            if (atEnd || atSep) {
                if (currentRow == row && currentCol == col) {
                    return data.substring(start, i).trim();
                }
                if (!atEnd && data.charAt(i) == ',') {
                    currentCol++;
                } else {
                    currentRow++;
                    currentCol = 0;
                }
                start = i + 1;
            }
        }
        return "";
    }

    public int sumColumn(int col) {
        int sum = 0;
        int numRows = countRows();
        // Skip header row (row 0)
        for (int row = 1; row < numRows; row++) {
            String cell = getCell(row, col);
            if (!cell.isEmpty()) {
                // Simple integer parsing
                int val = 0;
                boolean negative = false;
                int i = 0;
                if (cell.charAt(0) == '-') {
                    negative = true;
                    i = 1;
                }
                while (i < cell.length()) {
                    val = val * 10 + (cell.charAt(i) - '0');
                    i++;
                }
                if (negative) {
                    val = -val;
                }
                sum += val;
            }
        }
        return sum;
    }

    public String filterRows(String columnHeader, String value) {
        int numCols = countCols();
        int targetCol = -1;

        // Find the column index
        for (int c = 0; c < numCols; c++) {
            if (getCell(0, c).equals(columnHeader)) {
                targetCol = c;
                break;
            }
        }

        if (targetCol == -1) {
            return "Column not found: " + columnHeader;
        }

        StringBuilder result = new StringBuilder();
        // Add header row
        for (int c = 0; c < numCols; c++) {
            if (c > 0) {
                result.append(",");
            }
            result.append(getCell(0, c));
        }
        result.append("\n");

        // Add matching rows
        int numRows = countRows();
        for (int r = 1; r < numRows; r++) {
            if (getCell(r, targetCol).equals(value)) {
                for (int c = 0; c < numCols; c++) {
                    if (c > 0) {
                        result.append(",");
                    }
                    result.append(getCell(r, c));
                }
                result.append("\n");
            }
        }
        return result.toString();
    }

    public static void main(String[] args) {
        String csv = "name,age,city\nAlice,30,NYC\nBob,25,LA\nCarol,30,NYC\nDave,35,Chicago";

        CsvParser parser = new CsvParser(csv);

        System.out.println("Rows: " + parser.countRows());
        System.out.println("Cols: " + parser.countCols());

        // Test cell access
        System.out.println("Header[0]: " + parser.getCell(0, 0));
        System.out.println("Header[1]: " + parser.getCell(0, 1));
        System.out.println("Header[2]: " + parser.getCell(0, 2));
        System.out.println("Cell[1,0]: " + parser.getCell(1, 0));
        System.out.println("Cell[1,1]: " + parser.getCell(1, 1));
        System.out.println("Cell[2,0]: " + parser.getCell(2, 0));
        System.out.println("Cell[4,2]: " + parser.getCell(4, 2));

        // Test sum
        System.out.println("Sum of ages: " + parser.sumColumn(1));

        // Test filter
        System.out.println("--- Filter city=NYC ---");
        System.out.print(parser.filterRows("city", "NYC"));

        // Test with simpler CSV
        CsvParser p2 = new CsvParser("x,y\n1,2\n3,4\n5,6");
        System.out.println("Sum of x: " + p2.sumColumn(0));
        System.out.println("Sum of y: " + p2.sumColumn(1));

        System.out.println("CSV parser tests complete");
    }
}
