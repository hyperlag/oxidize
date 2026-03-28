public class EnumSwitch {
    enum Day { MON, TUE, WED, THU, FRI, SAT, SUN }

    static String classify(Day d) {
        switch (d) {
            case MON:
            case TUE:
            case WED:
            case THU:
            case FRI:
                return "Weekday";
            case SAT:
            case SUN:
                return "Weekend";
            default:
                return "Unknown";
        }
    }

    public static void main(String[] args) {
        System.out.println(classify(Day.MON));
        System.out.println(classify(Day.WED));
        System.out.println(classify(Day.SAT));
        System.out.println(classify(Day.SUN));
    }
}
