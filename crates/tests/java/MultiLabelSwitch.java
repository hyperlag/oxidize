class MultiLabelSwitch {
    public static void main(String[] args) {
        // Arrow-form switch expression with multi-labels
        for (int day = 1; day <= 7; day++) {
            String type = switch (day) {
                case 1, 2, 3, 4, 5 -> "Weekday";
                case 6, 7 -> "Weekend";
                default -> "Unknown";
            };
            if (day == 1 || day == 6) {
                System.out.println(day + " = " + type);
            }
        }

        // Arrow-form switch statement with multi-labels (int)
        int code = 2;
        switch (code) {
            case 1, 2 -> System.out.println("Good");
            case 3, 4 -> System.out.println("Average");
            default -> System.out.println("Other");
        }

        // Multi-label with int values in colon-form (fall-through)
        int x = 10;
        switch (x) {
            case 1, 2, 3:
                System.out.println("Low");
                break;
            case 10, 20, 30:
                System.out.println("Medium");
                break;
            default:
                System.out.println("High");
        }
    }
}
